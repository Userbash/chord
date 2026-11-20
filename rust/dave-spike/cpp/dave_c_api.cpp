#include "dave.h"

#include <openssl/bn.h>
#include <openssl/core_names.h>
#include <openssl/crypto.h>
#include <openssl/ec.h>
#include <openssl/ecdsa.h>
#include <openssl/evp.h>
#include <openssl/kdf.h>
#include <openssl/param_build.h>
#include <openssl/rand.h>

#include <algorithm>
#include <array>
#include <cstdlib>
#include <cstring>
#include <deque>
#include <map>
#include <memory>
#include <optional>
#include <set>
#include <string>
#include <utility>
#include <vector>

namespace {

constexpr uint16_t kDaveProtocolVersionV1 = 1;
constexpr uint16_t kMlsCiphersuiteId = 0x0001;
constexpr size_t kKemPrekeySlots = 8;
constexpr size_t kP256PubLen = 65;
constexpr size_t kP256SigLen = 64;
constexpr size_t kAes128KeyLen = 16;
constexpr size_t kGcmNonceLen = 12;
constexpr size_t kGcmTag16Len = 16;
constexpr size_t kDaveTag8Len = 8;
constexpr uint32_t kMaxGenerationGap = 250;
constexpr uint8_t kMarker0 = 0xFA;
constexpr uint8_t kMarker1 = 0xFA;
constexpr uint8_t kOpusSilence[3] = {0xF8, 0xFF, 0xFE};
constexpr uint8_t kFingerprintSalt[16] = {
    0x24, 0xca, 0xb1, 0x7a, 0x7a, 0xf8, 0xec, 0x2b,
    0x82, 0xb4, 0x12, 0xb9, 0x2d, 0xab, 0x19, 0x2e,
};

struct EvpPkeyDeleter {
  void operator()(EVP_PKEY* p) const { EVP_PKEY_free(p); }
};
using UniquePkey = std::unique_ptr<EVP_PKEY, EvpPkeyDeleter>;

uint8_t* alloc_copy_bytes(const std::vector<uint8_t>& src, size_t* out_len) {
  *out_len = src.size();
  if (src.empty()) return nullptr;
  auto* mem = static_cast<uint8_t*>(std::malloc(src.size()));
  if (!mem) return nullptr;
  std::memcpy(mem, src.data(), src.size());
  return mem;
}

uint64_t* alloc_copy_u64s(const std::vector<uint64_t>& src, size_t* out_len) {
  *out_len = src.size();
  if (src.empty()) return nullptr;
  auto* mem = static_cast<uint64_t*>(std::malloc(src.size() * sizeof(uint64_t)));
  if (!mem) return nullptr;
  std::memcpy(mem, src.data(), src.size() * sizeof(uint64_t));
  return mem;
}

void push_u16_be(std::vector<uint8_t>& out, uint16_t v) {
  out.push_back(static_cast<uint8_t>((v >> 8) & 0xFF));
  out.push_back(static_cast<uint8_t>(v & 0xFF));
}

void push_u32_be(std::vector<uint8_t>& out, uint32_t v) {
  for (int s = 24; s >= 0; s -= 8) out.push_back(static_cast<uint8_t>((v >> s) & 0xFF));
}

void push_u64_be(std::vector<uint8_t>& out, uint64_t v) {
  for (int s = 56; s >= 0; s -= 8) out.push_back(static_cast<uint8_t>((v >> s) & 0xFF));
}

void push_u64_le(std::vector<uint8_t>& out, uint64_t v) {
  for (int s = 0; s < 64; s += 8) out.push_back(static_cast<uint8_t>((v >> s) & 0xFF));
}

bool read_u16_be(const uint8_t*& p, const uint8_t* end, uint16_t& out) {
  if (end - p < 2) return false;
  out = (static_cast<uint16_t>(p[0]) << 8) | static_cast<uint16_t>(p[1]);
  p += 2;
  return true;
}

bool read_u64_be(const uint8_t*& p, const uint8_t* end, uint64_t& out) {
  if (end - p < 8) return false;
  out = 0;
  for (int i = 0; i < 8; ++i) out = (out << 8) | static_cast<uint64_t>(p[i]);
  p += 8;
  return true;
}

void write_vlbytes(std::vector<uint8_t>& out, const uint8_t* data, size_t len) {
  if (len < 64) {
    out.push_back(static_cast<uint8_t>(len));
  } else if (len < 16384) {
    uint16_t h = static_cast<uint16_t>(0x4000u | len);
    push_u16_be(out, h);
  } else {
    uint32_t h = static_cast<uint32_t>(0x80000000u | len);
    push_u32_be(out, h);
  }
  out.insert(out.end(), data, data + len);
}

void write_vlbytes_vec(std::vector<uint8_t>& out, const std::vector<uint8_t>& v) {
  write_vlbytes(out, v.data(), v.size());
}

bool read_vlbytes(const uint8_t*& p, const uint8_t* end, std::vector<uint8_t>& out) {
  if (p >= end) return false;
  uint8_t first = *p;
  uint8_t prefix = first >> 6;
  size_t len = 0;
  if (prefix == 0) {
    len = first & 0x3F;
    p += 1;
  } else if (prefix == 1) {
    if (end - p < 2) return false;
    len = (static_cast<size_t>(first & 0x3F) << 8) | static_cast<size_t>(p[1]);
    if (len < 64) return false;
    p += 2;
  } else if (prefix == 2) {
    if (end - p < 4) return false;
    len = (static_cast<size_t>(first & 0x3F) << 24) |
          (static_cast<size_t>(p[1]) << 16) |
          (static_cast<size_t>(p[2]) << 8) | static_cast<size_t>(p[3]);
    if (len < 16384) return false;
    p += 4;
  } else {
    return false;
  }
  if (static_cast<size_t>(end - p) < len) return false;
  out.assign(p, p + len);
  p += len;
  return true;
}

size_t leb128_size(uint64_t v) {
  size_t n = 1;
  while (v >= 0x80) {
    v >>= 7;
    ++n;
  }
  return n;
}

size_t write_leb128(uint64_t v, uint8_t* out) {
  size_t i = 0;
  while (v >= 0x80) {
    out[i++] = static_cast<uint8_t>(0x80u | (v & 0x7Fu));
    v >>= 7;
  }
  out[i++] = static_cast<uint8_t>(v);
  return i;
}

bool read_leb128(const uint8_t* buf, size_t len, uint32_t& value, size_t& used) {
  uint64_t acc = 0;
  uint32_t shift = 0;
  for (size_t i = 0; i < len && i < 5; ++i) {
    uint8_t b = buf[i];
    acc |= static_cast<uint64_t>(b & 0x7Fu) << shift;
    if ((b & 0x80u) == 0) {
      if (acc > 0xFFFFFFFFull || (i > 0 && b == 0)) return false;
      value = static_cast<uint32_t>(acc);
      used = i + 1;
      return true;
    }
    shift += 7;
  }
  return false;
}

std::array<uint8_t, 32> sha256_digest(const uint8_t* data, size_t len) {
  std::array<uint8_t, 32> out{};
  size_t out_len = 32;
  EVP_Q_digest(nullptr, "SHA256", nullptr, data, len, out.data(), &out_len);
  return out;
}

std::array<uint8_t, 32> hmac_sha256(const uint8_t* key,
                                    size_t key_len,
                                    const uint8_t* data,
                                    size_t data_len) {
  std::array<uint8_t, 32> out{};
  size_t out_len = 32;
  EVP_Q_mac(nullptr, "HMAC", nullptr, "SHA256", nullptr, key, key_len, data,
            data_len, out.data(), out.size(), &out_len);
  return out;
}

std::array<uint8_t, 32> hkdf_extract_sha256(const uint8_t* salt,
                                            size_t salt_len,
                                            const uint8_t* ikm,
                                            size_t ikm_len) {
  static const uint8_t kZeroSalt[32] = {0};
  if (salt_len == 0) {
    return hmac_sha256(kZeroSalt, 32, ikm, ikm_len);
  }
  return hmac_sha256(salt, salt_len, ikm, ikm_len);
}

bool hkdf_expand_sha256(const uint8_t* prk,
                        size_t prk_len,
                        const uint8_t* info,
                        size_t info_len,
                        uint8_t* out,
                        size_t out_len) {
  if (out_len == 0) return true;
  EVP_KDF* kdf = EVP_KDF_fetch(nullptr, "HKDF", nullptr);
  if (!kdf) return false;
  EVP_KDF_CTX* ctx = EVP_KDF_CTX_new(kdf);
  EVP_KDF_free(kdf);
  if (!ctx) return false;

  int mode = EVP_KDF_HKDF_MODE_EXPAND_ONLY;
  char digest_name[] = "SHA256";
  OSSL_PARAM params[5];
  params[0] = OSSL_PARAM_construct_int(OSSL_KDF_PARAM_MODE, &mode);
  params[1] = OSSL_PARAM_construct_utf8_string(OSSL_KDF_PARAM_DIGEST, digest_name, 0);
  params[2] = OSSL_PARAM_construct_octet_string(
      OSSL_KDF_PARAM_KEY, const_cast<uint8_t*>(prk), prk_len);
  params[3] = OSSL_PARAM_construct_octet_string(
      OSSL_KDF_PARAM_INFO, const_cast<uint8_t*>(info), info_len);
  params[4] = OSSL_PARAM_construct_end();

  bool ok = EVP_KDF_derive(ctx, out, out_len, params) == 1;
  EVP_KDF_CTX_free(ctx);
  return ok;
}

std::vector<uint8_t> mls_expand_with_label(const uint8_t* secret,
                                           size_t secret_len,
                                           const std::string& label,
                                           const uint8_t* context,
                                           size_t context_len,
                                           size_t length) {
  std::string full_label = "MLS 1.0 " + label;
  std::vector<uint8_t> info;
  push_u16_be(info, static_cast<uint16_t>(length));
  write_vlbytes(info, reinterpret_cast<const uint8_t*>(full_label.data()), full_label.size());
  write_vlbytes(info, context, context_len);
  std::vector<uint8_t> out(length, 0);
  hkdf_expand_sha256(secret, secret_len, info.data(), info.size(), out.data(), length);
  return out;
}

std::vector<uint8_t> mls_derive_tree_secret(const uint8_t* secret,
                                            size_t secret_len,
                                            const std::string& label,
                                            uint32_t generation,
                                            size_t length) {
  uint8_t gen_be[4] = {
      static_cast<uint8_t>((generation >> 24) & 0xFF),
      static_cast<uint8_t>((generation >> 16) & 0xFF),
      static_cast<uint8_t>((generation >> 8) & 0xFF),
      static_cast<uint8_t>(generation & 0xFF),
  };
  return mls_expand_with_label(secret, secret_len, label, gen_be, 4, length);
}

UniquePkey generate_p256_key() {
  return UniquePkey(EVP_EC_gen("P-256"));
}

bool export_p256_pub_65(EVP_PKEY* pkey, std::array<uint8_t, kP256PubLen>& out) {
  size_t len = 0;
  if (EVP_PKEY_get_octet_string_param(pkey, OSSL_PKEY_PARAM_PUB_KEY, out.data(),
                                      out.size(), &len) != 1) {
    return false;
  }
  return len == kP256PubLen && out[0] == 0x04;
}

UniquePkey import_p256_pub_65(const uint8_t* pub65, size_t len) {
  if (len != kP256PubLen || pub65[0] != 0x04) return nullptr;
  EVP_PKEY_CTX* ctx = EVP_PKEY_CTX_new_from_name(nullptr, "EC", nullptr);
  if (!ctx) return nullptr;
  EVP_PKEY* pkey = nullptr;
  if (EVP_PKEY_fromdata_init(ctx) == 1) {
    char group[] = "prime256v1";
    OSSL_PARAM params[3];
    params[0] = OSSL_PARAM_construct_utf8_string(OSSL_PKEY_PARAM_GROUP_NAME, group, 0);
    params[1] = OSSL_PARAM_construct_octet_string(
        OSSL_PKEY_PARAM_PUB_KEY, const_cast<uint8_t*>(pub65), kP256PubLen);
    params[2] = OSSL_PARAM_construct_end();
    EVP_PKEY_fromdata(ctx, &pkey, EVP_PKEY_PUBLIC_KEY, params);
  }
  EVP_PKEY_CTX_free(ctx);
  return UniquePkey(pkey);
}

bool ecdsa_sign_fixed64(EVP_PKEY* priv,
                        const uint8_t* msg,
                        size_t msg_len,
                        std::array<uint8_t, kP256SigLen>& out_sig) {
  EVP_MD_CTX* mctx = EVP_MD_CTX_new();
  if (!mctx) return false;
  size_t der_len = 0;
  bool ok = EVP_DigestSignInit(mctx, nullptr, EVP_sha256(), nullptr, priv) == 1 &&
            EVP_DigestSign(mctx, nullptr, &der_len, msg, msg_len) == 1;
  std::vector<uint8_t> der(der_len);
  if (ok) {
    ok = EVP_DigestSign(mctx, der.data(), &der_len, msg, msg_len) == 1;
  }
  EVP_MD_CTX_free(mctx);
  if (!ok) return false;

  const uint8_t* p = der.data();
  ECDSA_SIG* sig = d2i_ECDSA_SIG(nullptr, &p, static_cast<long>(der_len));
  if (!sig) return false;
  const BIGNUM* r = nullptr;
  const BIGNUM* s = nullptr;
  ECDSA_SIG_get0(sig, &r, &s);
  ok = BN_bn2binpad(r, out_sig.data(), 32) == 32 &&
       BN_bn2binpad(s, out_sig.data() + 32, 32) == 32;
  ECDSA_SIG_free(sig);
  return ok;
}

bool ecdsa_verify_fixed64(const uint8_t* pub65,
                          const uint8_t* msg,
                          size_t msg_len,
                          const uint8_t* sig64,
                          size_t sig_len) {
  if (sig_len != kP256SigLen) return false;
  UniquePkey pub = import_p256_pub_65(pub65, kP256PubLen);
  if (!pub) return false;

  BIGNUM* r = BN_bin2bn(sig64, 32, nullptr);
  BIGNUM* s = BN_bin2bn(sig64 + 32, 32, nullptr);
  ECDSA_SIG* sig = ECDSA_SIG_new();
  if (!r || !s || !sig || ECDSA_SIG_set0(sig, r, s) != 1) {
    BN_free(r);
    BN_free(s);
    ECDSA_SIG_free(sig);
    return false;
  }
  uint8_t* der = nullptr;
  int der_len = i2d_ECDSA_SIG(sig, &der);
  ECDSA_SIG_free(sig);
  if (der_len <= 0) return false;

  EVP_MD_CTX* mctx = EVP_MD_CTX_new();
  bool ok = mctx &&
            EVP_DigestVerifyInit(mctx, nullptr, EVP_sha256(), nullptr, pub.get()) == 1 &&
            EVP_DigestVerify(mctx, der, static_cast<size_t>(der_len), msg, msg_len) == 1;
  EVP_MD_CTX_free(mctx);
  OPENSSL_free(der);
  return ok;
}

bool p256_ecdh_shared(EVP_PKEY* priv,
                      const uint8_t* peer_pub65,
                      std::array<uint8_t, 32>& out_zz) {
  UniquePkey peer = import_p256_pub_65(peer_pub65, kP256PubLen);
  if (!peer) return false;
  EVP_PKEY_CTX* ctx = EVP_PKEY_CTX_new(priv, nullptr);
  if (!ctx) return false;
  size_t len = 0;
  bool ok = EVP_PKEY_derive_init(ctx) == 1 &&
            EVP_PKEY_derive_set_peer(ctx, peer.get()) == 1 &&
            EVP_PKEY_derive(ctx, nullptr, &len) == 1 && len == 32 &&
            EVP_PKEY_derive(ctx, out_zz.data(), &len) == 1;
  EVP_PKEY_CTX_free(ctx);
  return ok;
}

bool aes128_gcm_seal_var_tag(const uint8_t* key16,
                             const uint8_t* nonce12,
                             const uint8_t* aad,
                             size_t aad_len,
                             const uint8_t* pt,
                             size_t pt_len,
                             uint8_t* ct_out,
                             uint8_t* tag_out,
                             size_t tag_len) {
  EVP_CIPHER_CTX* ctx = EVP_CIPHER_CTX_new();
  if (!ctx) return false;
  int outl = 0;
  int finl = 0;
  bool ok = EVP_EncryptInit_ex(ctx, EVP_aes_128_gcm(), nullptr, nullptr, nullptr) == 1 &&
            EVP_CIPHER_CTX_ctrl(ctx, EVP_CTRL_AEAD_SET_IVLEN, kGcmNonceLen, nullptr) == 1 &&
            EVP_EncryptInit_ex(ctx, nullptr, nullptr, key16, nonce12) == 1;
  if (ok && aad_len > 0) {
    ok = EVP_EncryptUpdate(ctx, nullptr, &outl, aad, static_cast<int>(aad_len)) == 1;
  }
  if (ok && pt_len > 0) {
    ok = EVP_EncryptUpdate(ctx, ct_out, &outl, pt, static_cast<int>(pt_len)) == 1;
  }
  if (ok) {
    ok = EVP_EncryptFinal_ex(ctx, ct_out + outl, &finl) == 1 &&
         EVP_CIPHER_CTX_ctrl(ctx, EVP_CTRL_AEAD_GET_TAG, static_cast<int>(tag_len),
                             tag_out) == 1;
  }
  EVP_CIPHER_CTX_free(ctx);
  return ok;
}

bool aes128_gcm_open_var_tag(const uint8_t* key16,
                             const uint8_t* nonce12,
                             const uint8_t* aad,
                             size_t aad_len,
                             const uint8_t* ct,
                             size_t ct_len,
                             const uint8_t* tag,
                             size_t tag_len,
                             uint8_t* pt_out) {
  EVP_CIPHER_CTX* ctx = EVP_CIPHER_CTX_new();
  if (!ctx) return false;
  int outl = 0;
  int finl = 0;
  // Authenticate full 16B tag or truncated 8B tag cleanly via EVP_CTRL_AEAD_SET_TAG
  bool ok = EVP_DecryptInit_ex(ctx, EVP_aes_128_gcm(), nullptr, nullptr, nullptr) == 1 &&
            EVP_CIPHER_CTX_ctrl(ctx, EVP_CTRL_AEAD_SET_IVLEN, kGcmNonceLen, nullptr) == 1 &&
            EVP_DecryptInit_ex(ctx, nullptr, nullptr, key16, nonce12) == 1;
  if (ok && aad_len > 0) {
    ok = EVP_DecryptUpdate(ctx, nullptr, &outl, aad, static_cast<int>(aad_len)) == 1;
  }
  std::vector<uint8_t> scratch(ct_len);
  if (ok && ct_len > 0) {
    ok = EVP_DecryptUpdate(ctx, scratch.data(), &outl, ct, static_cast<int>(ct_len)) == 1;
  }
  if (ok) {
    ok = EVP_CIPHER_CTX_ctrl(ctx, EVP_CTRL_AEAD_SET_TAG, static_cast<int>(tag_len),
                             const_cast<uint8_t*>(tag)) == 1 &&
         EVP_DecryptFinal_ex(ctx, scratch.data() + outl, &finl) == 1;
  }
  EVP_CIPHER_CTX_free(ctx);
  if (!ok) {
    OPENSSL_cleanse(scratch.data(), scratch.size());
    return false;
  }
  if (ct_len > 0) std::memcpy(pt_out, scratch.data(), ct_len);
  return true;
}

void derive_hpke_key_nonce(const std::array<uint8_t, 32>& zz,
                           const uint8_t* eph_pub65,
                           const uint8_t* recip_pub65,
                           std::array<uint8_t, 16>& key16,
                           std::array<uint8_t, 12>& nonce12) {
  static const char kSalt[] = "DAVE-MLS-HPKE-v1";
  auto prk = hkdf_extract_sha256(reinterpret_cast<const uint8_t*>(kSalt),
                                 sizeof(kSalt) - 1, zz.data(), zz.size());
  std::vector<uint8_t> ctx_key;
  ctx_key.insert(ctx_key.end(), {'k', 'e', 'y'});
  ctx_key.insert(ctx_key.end(), eph_pub65, eph_pub65 + kP256PubLen);
  ctx_key.insert(ctx_key.end(), recip_pub65, recip_pub65 + kP256PubLen);
  hkdf_expand_sha256(prk.data(), prk.size(), ctx_key.data(), ctx_key.size(),
                     key16.data(), key16.size());

  std::vector<uint8_t> ctx_nonce;
  ctx_nonce.insert(ctx_nonce.end(), {'n', 'o', 'n', 'c', 'e'});
  ctx_nonce.insert(ctx_nonce.end(), eph_pub65, eph_pub65 + kP256PubLen);
  ctx_nonce.insert(ctx_nonce.end(), recip_pub65, recip_pub65 + kP256PubLen);
  hkdf_expand_sha256(prk.data(), prk.size(), ctx_nonce.data(), ctx_nonce.size(),
                     nonce12.data(), nonce12.size());
  OPENSSL_cleanse(prk.data(), prk.size());
}

bool hpke_seal(const uint8_t* recip_pub65,
               const std::vector<uint8_t>& aad,
               const std::vector<uint8_t>& pt,
               std::array<uint8_t, kP256PubLen>& out_eph_pub,
               std::vector<uint8_t>& out_ct_and_tag) {
  UniquePkey eph = generate_p256_key();
  if (!eph || !export_p256_pub_65(eph.get(), out_eph_pub)) return false;
  std::array<uint8_t, 32> zz{};
  if (!p256_ecdh_shared(eph.get(), recip_pub65, zz)) return false;

  std::array<uint8_t, 16> key{};
  std::array<uint8_t, 12> nonce{};
  derive_hpke_key_nonce(zz, out_eph_pub.data(), recip_pub65, key, nonce);
  OPENSSL_cleanse(zz.data(), zz.size());

  out_ct_and_tag.resize(pt.size() + kGcmTag16Len);
  bool ok = aes128_gcm_seal_var_tag(
      key.data(), nonce.data(), aad.data(), aad.size(), pt.data(), pt.size(),
      out_ct_and_tag.data(), out_ct_and_tag.data() + pt.size(), kGcmTag16Len);
  OPENSSL_cleanse(key.data(), key.size());
  return ok;
}

bool hpke_open(EVP_PKEY* recip_priv,
               const std::array<uint8_t, kP256PubLen>& recip_pub65,
               const uint8_t* eph_pub65,
               const std::vector<uint8_t>& aad,
               const std::vector<uint8_t>& ct_and_tag,
               std::vector<uint8_t>& out_pt) {
  if (ct_and_tag.size() < kGcmTag16Len) return false;
  std::array<uint8_t, 32> zz{};
  if (!p256_ecdh_shared(recip_priv, eph_pub65, zz)) return false;

  std::array<uint8_t, 16> key{};
  std::array<uint8_t, 12> nonce{};
  derive_hpke_key_nonce(zz, eph_pub65, recip_pub65.data(), key, nonce);
  OPENSSL_cleanse(zz.data(), zz.size());

  size_t pt_len = ct_and_tag.size() - kGcmTag16Len;
  out_pt.resize(pt_len);
  bool ok = aes128_gcm_open_var_tag(
      key.data(), nonce.data(), aad.data(), aad.size(), ct_and_tag.data(),
      pt_len, ct_and_tag.data() + pt_len, kGcmTag16Len, out_pt.data());
  OPENSSL_cleanse(key.data(), key.size());
  return ok;
}

struct KemPubSlot {
  uint8_t slot_id;
  std::array<uint8_t, kP256PubLen> pub;
};

struct MemberNode {
  uint64_t user_id;
  std::array<uint8_t, kP256PubLen> sig_pub;
  std::vector<KemPubSlot> kem_slots;
};

struct ParsedKeyPackage {
  uint64_t user_id = 0;
  std::array<uint8_t, kP256PubLen> sig_pub{};
  std::vector<KemPubSlot> kem_slots;
};

bool parse_and_verify_key_package(const std::vector<uint8_t>& raw, ParsedKeyPackage& out) {
  const uint8_t* p = raw.data();
  const uint8_t* end = p + raw.size();
  std::vector<uint8_t> body;
  std::vector<uint8_t> sig;
  if (!read_vlbytes(p, end, body) || !read_vlbytes(p, end, sig) || p != end) return false;

  const uint8_t* bp = body.data();
  const uint8_t* bend = bp + body.size();
  uint16_t ver = 0;
  uint16_t suite = 0;
  uint16_t cred_type = 0;
  std::vector<uint8_t> uid_bytes;
  std::vector<uint8_t> sig_pub_vec;
  uint64_t not_before = 0;
  uint64_t not_after = 0;
  if (!read_u16_be(bp, bend, ver) || ver != kDaveProtocolVersionV1) return false;
  if (!read_u16_be(bp, bend, suite) || suite != kMlsCiphersuiteId) return false;
  if (!read_u16_be(bp, bend, cred_type) || cred_type != 1) return false;
  if (!read_vlbytes(bp, bend, uid_bytes) || uid_bytes.size() != 8) return false;
  if (!read_vlbytes(bp, bend, sig_pub_vec) || sig_pub_vec.size() != kP256PubLen) return false;
  if (bp >= bend) return false;
  uint8_t slot_count = *bp++;
  if (slot_count == 0 || slot_count > 16) return false;

  out.kem_slots.clear();
  for (uint8_t i = 0; i < slot_count; ++i) {
    if (bp >= bend) return false;
    uint8_t sid = *bp++;
    std::vector<uint8_t> kpub;
    if (!read_vlbytes(bp, bend, kpub) || kpub.size() != kP256PubLen) return false;
    KemPubSlot slot{};
    slot.slot_id = sid;
    std::memcpy(slot.pub.data(), kpub.data(), kP256PubLen);
    out.kem_slots.push_back(slot);
  }
  if (!read_u64_be(bp, bend, not_before) || !read_u64_be(bp, bend, not_after) || bp != bend) {
    return false;
  }
  const uint8_t* up = uid_bytes.data();
  read_u64_be(up, up + 8, out.user_id);
  if (out.user_id == 0) return false;
  std::memcpy(out.sig_pub.data(), sig_pub_vec.data(), kP256PubLen);

  std::vector<uint8_t> tbs = {'M', 'L', 'S', ' ', '1', '.', '0', ' ', 'K', 'P'};
  tbs.insert(tbs.end(), body.begin(), body.end());
  return ecdsa_verify_fixed64(out.sig_pub.data(), tbs.data(), tbs.size(), sig.data(), sig.size());
}

void serialize_roster(const std::vector<MemberNode>& roster, std::vector<uint8_t>& out) {
  push_u16_be(out, static_cast<uint16_t>(roster.size()));
  for (const auto& m : roster) {
    push_u64_be(out, m.user_id);
    write_vlbytes(out, m.sig_pub.data(), m.sig_pub.size());
    out.push_back(static_cast<uint8_t>(m.kem_slots.size()));
    for (const auto& s : m.kem_slots) {
      out.push_back(s.slot_id);
      write_vlbytes(out, s.pub.data(), s.pub.size());
    }
  }
}

bool deserialize_roster(const uint8_t*& p, const uint8_t* end, std::vector<MemberNode>& out) {
  uint16_t count = 0;
  if (!read_u16_be(p, end, count) || count == 0 || count > 64) return false;
  out.clear();
  for (uint16_t i = 0; i < count; ++i) {
    MemberNode m{};
    std::vector<uint8_t> spub;
    if (!read_u64_be(p, end, m.user_id) || m.user_id == 0) return false;
    if (!read_vlbytes(p, end, spub) || spub.size() != kP256PubLen) return false;
    std::memcpy(m.sig_pub.data(), spub.data(), kP256PubLen);
    if (p >= end) return false;
    uint8_t sc = *p++;
    if (sc > 16) return false;
    for (uint8_t j = 0; j < sc; ++j) {
      if (p >= end) return false;
      KemPubSlot s{};
      s.slot_id = *p++;
      std::vector<uint8_t> kp;
      if (!read_vlbytes(p, end, kp) || kp.size() != kP256PubLen) return false;
      std::memcpy(s.pub.data(), kp.data(), kP256PubLen);
      m.kem_slots.push_back(s);
    }
    out.push_back(std::move(m));
  }
  return true;
}

std::vector<uint8_t> build_group_context_bytes(uint64_t group_id,
                                               uint64_t epoch,
                                               const std::array<uint8_t, 32>& tree_hash,
                                               const std::array<uint8_t, 32>& transcript_hash,
                                               const std::vector<uint8_t>& ext_sender_raw) {
  std::vector<uint8_t> gc;
  push_u16_be(gc, kDaveProtocolVersionV1);
  push_u16_be(gc, kMlsCiphersuiteId);
  std::vector<uint8_t> gid;
  push_u64_be(gid, group_id);
  write_vlbytes_vec(gc, gid);
  push_u64_be(gc, epoch);
  write_vlbytes(gc, tree_hash.data(), 32);
  write_vlbytes(gc, transcript_hash.data(), 32);
  write_vlbytes_vec(gc, ext_sender_raw);
  return gc;
}

struct EpochDerived {
  std::array<uint8_t, 32> joiner_secret{};
  std::array<uint8_t, 16> welcome_key{};
  std::array<uint8_t, 12> welcome_nonce{};
  std::array<uint8_t, 32> exporter_secret{};
  std::array<uint8_t, 32> epoch_authenticator{};
  std::array<uint8_t, 32> confirmation_tag{};
  std::array<uint8_t, 32> init_secret{};
  ~EpochDerived() {
    OPENSSL_cleanse(joiner_secret.data(), joiner_secret.size());
    OPENSSL_cleanse(welcome_key.data(), welcome_key.size());
    OPENSSL_cleanse(exporter_secret.data(), exporter_secret.size());
    OPENSSL_cleanse(init_secret.data(), init_secret.size());
  }
};

EpochDerived derive_epoch_from_joiner(const std::array<uint8_t, 32>& joiner_secret,
                                      const std::vector<uint8_t>& group_context,
                                      const std::array<uint8_t, 32>& transcript_hash) {
  EpochDerived d{};
  d.joiner_secret = joiner_secret;
  uint8_t zero32[32] = {0};
  auto member_secret = hkdf_extract_sha256(joiner_secret.data(), 32, zero32, 32);
  auto welcome_secret = mls_expand_with_label(member_secret.data(), 32, "welcome", nullptr, 0, 32);
  auto wkey = mls_expand_with_label(welcome_secret.data(), 32, "key", nullptr, 0, 16);
  auto wnonce = mls_expand_with_label(welcome_secret.data(), 32, "nonce", nullptr, 0, 12);
  std::memcpy(d.welcome_key.data(), wkey.data(), 16);
  std::memcpy(d.welcome_nonce.data(), wnonce.data(), 12);

  auto epoch_secret = mls_expand_with_label(
      member_secret.data(), 32, "epoch", group_context.data(), group_context.size(), 32);
  auto exp = mls_expand_with_label(epoch_secret.data(), 32, "exporter", nullptr, 0, 32);
  auto auth = mls_expand_with_label(epoch_secret.data(), 32, "authentication", nullptr, 0, 32);
  auto conf_key = mls_expand_with_label(epoch_secret.data(), 32, "confirm", nullptr, 0, 32);
  auto init = mls_expand_with_label(epoch_secret.data(), 32, "init", nullptr, 0, 32);

  std::memcpy(d.exporter_secret.data(), exp.data(), 32);
  std::memcpy(d.epoch_authenticator.data(), auth.data(), 32);
  std::memcpy(d.init_secret.data(), init.data(), 32);
  d.confirmation_tag = hmac_sha256(conf_key.data(), 32, transcript_hash.data(), 32);

  OPENSSL_cleanse(member_secret.data(), member_secret.size());
  OPENSSL_cleanse(epoch_secret.data(), epoch_secret.size());
  OPENSSL_cleanse(conf_key.data(), conf_key.size());
  return d;
}

EpochDerived derive_epoch_from_commit(const std::array<uint8_t, 32>& prev_init_secret,
                                      const std::array<uint8_t, 32>& commit_secret,
                                      const std::vector<uint8_t>& group_context,
                                      const std::array<uint8_t, 32>& transcript_hash) {
  auto inter = hkdf_extract_sha256(prev_init_secret.data(), 32, commit_secret.data(), 32);
  auto joiner_vec = mls_expand_with_label(
      inter.data(), 32, "joiner", group_context.data(), group_context.size(), 32);
  OPENSSL_cleanse(inter.data(), inter.size());
  std::array<uint8_t, 32> joiner{};
  std::memcpy(joiner.data(), joiner_vec.data(), 32);
  OPENSSL_cleanse(joiner_vec.data(), joiner_vec.size());
  return derive_epoch_from_joiner(joiner, group_context, transcript_hash);
}

std::array<uint8_t, 16> export_user_media_base_secret(
    const std::array<uint8_t, 32>& exporter_secret, uint64_t user_id) {
  static const std::string kLabel = "Discord Secure Frames v0";
  auto derived = mls_expand_with_label(exporter_secret.data(), 32, kLabel, nullptr, 0, 32);
  std::vector<uint8_t> uid_le;
  push_u64_le(uid_le, user_id);
  auto ctx_hash = sha256_digest(uid_le.data(), uid_le.size());
  auto out_vec = mls_expand_with_label(derived.data(), 32, "exported", ctx_hash.data(), 32, 16);
  OPENSSL_cleanse(derived.data(), derived.size());
  std::array<uint8_t, 16> out{};
  std::memcpy(out.data(), out_vec.data(), 16);
  OPENSSL_cleanse(out_vec.data(), out_vec.size());
  return out;
}

struct QueuedProposal {
  std::array<uint8_t, 16> ref{};
  bool is_add = true;
  ParsedKeyPackage added;
  uint64_t removed_user_id = 0;
};

struct Range {
  size_t offset;
  size_t size;
};

std::vector<Range> h264_unencrypted_ranges(const uint8_t* frame, size_t len) {
  std::vector<Range> ranges;
  size_t i = 0;
  while (i + 4 <= len) {
    size_t sc_len = 0;
    if (frame[i] == 0 && frame[i + 1] == 0 && frame[i + 2] == 1) {
      sc_len = 3;
    } else if (i + 4 < len && frame[i] == 0 && frame[i + 1] == 0 &&
               frame[i + 2] == 0 && frame[i + 3] == 1) {
      sc_len = 4;
    }
    if (sc_len > 0 && i + sc_len < len) {
      ranges.push_back(Range{i, sc_len + 1});
      i += sc_len + 1;
    } else {
      ++i;
    }
  }
  return ranges;
}

bool ciphertext_has_h26x_start_code(const std::vector<Range>& ranges,
                                    const uint8_t* out_frame,
                                    size_t encrypted_payload_end) {
  if (ranges.empty()) return false;
  size_t pos = 0;
  for (size_t idx = 0; idx <= ranges.size(); ++idx) {
    size_t seg_end = (idx < ranges.size()) ? ranges[idx].offset : encrypted_payload_end;
    if (seg_end > pos + 2) {
      for (size_t k = pos; k + 2 < seg_end; ++k) {
        if (out_frame[k] == 0 && out_frame[k + 1] == 0 && out_frame[k + 2] <= 3) {
          return true;
        }
      }
    }
    if (idx < ranges.size()) pos = ranges[idx].offset + ranges[idx].size;
  }
  return false;
}

uint32_t compute_wrapped_generation(uint32_t oldest, uint32_t gen_u8) {
  uint32_t rem = oldest & 0xFFu;
  uint32_t delta = (gen_u8 - rem) & 0xFFu;
  uint64_t sum = static_cast<uint64_t>(oldest) + static_cast<uint64_t>(delta);
  return sum > 0xFFFFFFFFull ? 0xFFFFFFFFu : static_cast<uint32_t>(sum);
}

uint64_t compute_wrapped_big_nonce(uint32_t generation, uint32_t truncated_nonce) {
  uint64_t low24 = static_cast<uint64_t>(truncated_nonce) & 0x00FFFFFFull;
  return (static_cast<uint64_t>(generation) << 24) | low24;
}

class HashRatchetImpl {
 public:
  explicit HashRatchetImpl(std::array<uint8_t, 16> base)
      : next_secret_(base.begin(), base.end()) {}

  ~HashRatchetImpl() {
    OPENSSL_cleanse(next_secret_.data(), next_secret_.size());
    for (auto& kv : cache_) {
      OPENSSL_cleanse(kv.second.first.data(), kv.second.first.size());
    }
  }

  bool get(uint32_t generation,
           std::array<uint8_t, 16>& out_key,
           std::array<uint8_t, 12>& out_nonce) {
    auto it = cache_.find(generation);
    if (it != cache_.end()) {
      out_key = it->second.first;
      out_nonce = it->second.second;
      return true;
    }
    if (generation < next_gen_) return false;
    if (generation > next_gen_ + kMaxGenerationGap) return false;
    while (next_gen_ <= generation) {
      uint32_t g = next_gen_;
      auto k = mls_derive_tree_secret(next_secret_.data(), next_secret_.size(), "key", g, 16);
      auto n = mls_derive_tree_secret(next_secret_.data(), next_secret_.size(), "nonce", g, 12);
      auto s = mls_derive_tree_secret(next_secret_.data(), next_secret_.size(), "secret", g, 32);
      OPENSSL_cleanse(next_secret_.data(), next_secret_.size());
      next_secret_ = std::move(s);
      std::array<uint8_t, 16> ka{};
      std::array<uint8_t, 12> na{};
      std::memcpy(ka.data(), k.data(), 16);
      std::memcpy(na.data(), n.data(), 12);
      OPENSSL_cleanse(k.data(), k.size());
      cache_[g] = {ka, na};
      ++next_gen_;
    }
    out_key = cache_[generation].first;
    out_nonce = cache_[generation].second;
    return true;
  }

  void prune_before(uint32_t oldest_keep) {
    for (auto it = cache_.begin(); it != cache_.end();) {
      if (it->first < oldest_keep) {
        OPENSSL_cleanse(it->second.first.data(), it->second.first.size());
        it = cache_.erase(it);
      } else {
        ++it;
      }
    }
  }

 private:
  std::vector<uint8_t> next_secret_;
  uint32_t next_gen_ = 0;
  std::map<uint32_t, std::pair<std::array<uint8_t, 16>, std::array<uint8_t, 12>>> cache_;
};

class AntiReplayBitmap {
 public:
  bool can_accept(uint64_t seq) const {
    if (!initialized_) return true;
    if (seq > highest_) return true;
    uint64_t diff = highest_ - seq;
    if (diff >= 1024) return false;
    size_t word = static_cast<size_t>(diff / 64);
    uint64_t mask = 1ull << (diff % 64);
    return (bitmap_[word] & mask) == 0;
  }

  void accept(uint64_t seq) {
    if (!initialized_) {
      initialized_ = true;
      highest_ = seq;
      bitmap_.fill(0);
      bitmap_[0] = 1ull;
      return;
    }
    if (seq > highest_) {
      uint64_t shift = seq - highest_;
      if (shift >= 1024) {
        bitmap_.fill(0);
      } else {
        size_t word_shift = static_cast<size_t>(shift / 64);
        uint32_t bit_shift = static_cast<uint32_t>(shift % 64);
        std::array<uint64_t, 16> next{};
        for (size_t i = 0; i < 16; ++i) {
          if (i >= word_shift) {
            uint64_t v = bitmap_[i - word_shift] << bit_shift;
            if (bit_shift > 0 && i > word_shift) {
              v |= bitmap_[i - word_shift - 1] >> (64 - bit_shift);
            }
            next[i] = v;
          }
        }
        bitmap_ = next;
      }
      highest_ = seq;
      bitmap_[0] |= 1ull;
    } else {
      uint64_t diff = highest_ - seq;
      if (diff < 1024) {
        bitmap_[static_cast<size_t>(diff / 64)] |= (1ull << (diff % 64));
      }
    }
  }

 private:
  bool initialized_ = false;
  uint64_t highest_ = 0;
  std::array<uint64_t, 16> bitmap_{};
};

}  // namespace

struct OpaqueDaveKeyRatchet {
  std::array<uint8_t, 16> base_secret{};
  ~OpaqueDaveKeyRatchet() { OPENSSL_cleanse(base_secret.data(), base_secret.size()); }
};

struct OpaqueDaveCommitResult {
  bool failed = false;
  bool ignored = false;
  std::vector<uint64_t> roster;
};

struct OpaqueDaveWelcomeResult {
  bool failed = false;
  std::vector<uint64_t> roster;
};

struct OpaqueDaveSession {
  uint16_t version = kDaveProtocolVersionV1;
  uint64_t group_id = 0;
  uint64_t self_user_id = 0;
  UniquePkey sig_key;
  std::array<uint8_t, kP256PubLen> sig_pub{};
  std::map<uint8_t, std::pair<std::array<uint8_t, kP256PubLen>, UniquePkey>> own_kem_slots;

  bool has_external_sender = false;
  std::array<uint8_t, kP256PubLen> external_sender_pub{};
  std::vector<uint8_t> external_sender_raw;

  bool in_established_group = false;
  uint64_t epoch = 0;
  std::vector<MemberNode> roster;
  std::array<uint8_t, 32> init_secret{};
  std::array<uint8_t, 32> exporter_secret{};
  std::array<uint8_t, 32> epoch_authenticator{};
  std::array<uint8_t, 32> transcript_hash{};

  std::vector<QueuedProposal> pending_proposals;

  struct StagedOwnCommit {
    std::vector<uint8_t> raw_commit;
    uint64_t new_epoch = 0;
    std::vector<MemberNode> new_roster;
    EpochDerived derived;
    std::array<uint8_t, 32> new_transcript{};
  };
  std::unique_ptr<StagedOwnCommit> staged_own_commit;

  ~OpaqueDaveSession() {
    OPENSSL_cleanse(init_secret.data(), init_secret.size());
    OPENSSL_cleanse(exporter_secret.data(), exporter_secret.size());
    OPENSSL_cleanse(epoch_authenticator.data(), epoch_authenticator.size());
  }

  void regenerate_kem_slots() {
    own_kem_slots.clear();
    for (uint8_t i = 0; i < kKemPrekeySlots; ++i) {
      UniquePkey k = generate_p256_key();
      std::array<uint8_t, kP256PubLen> pub{};
      export_p256_pub_65(k.get(), pub);
      own_kem_slots[i] = {pub, std::move(k)};
    }
  }

  MemberNode self_member_node() const {
    MemberNode m{};
    m.user_id = self_user_id;
    m.sig_pub = sig_pub;
    for (const auto& kv : own_kem_slots) {
      m.kem_slots.push_back(KemPubSlot{kv.first, kv.second.first});
    }
    return m;
  }
};

struct OpaqueDaveEncryptor {
  std::unique_ptr<HashRatchetImpl> ratchet;
  uint32_t current_gen = 0;
  uint32_t truncated_nonce = 0;
  std::array<uint8_t, 16> current_key{};
  bool has_key = false;
  bool passthrough = false;
  ~OpaqueDaveEncryptor() { OPENSSL_cleanse(current_key.data(), current_key.size()); }
};

struct CipherManagerEntry {
  std::unique_ptr<HashRatchetImpl> ratchet;
  uint32_t oldest_gen = 0;
  uint32_t newest_gen = 0;
  AntiReplayBitmap replay;
};

struct OpaqueDaveDecryptor {
  std::deque<CipherManagerEntry> managers;
  bool passthrough = false;
};

struct ExternalSenderPrivate {
  UniquePkey sig_key;
  std::array<uint8_t, kP256PubLen> sig_pub{};
};

extern "C" {

uint16_t daveMaxSupportedProtocolVersion(void) { return kDaveProtocolVersionV1; }

void daveFree(void* ptr) { std::free(ptr); }

DAVESessionHandle daveSessionCreate(void*, const char*, DAVEMLSFailureCallback, void*) {
  return new OpaqueDaveSession();
}

void daveSessionDestroy(DAVESessionHandle session) { delete session; }

bool daveSessionInit(DAVESessionHandle session,
                     uint16_t version,
                     uint64_t group_id,
                     const char* self_user_id) {
  if (!session || version != kDaveProtocolVersionV1 || !self_user_id) return false;
  uint64_t uid = std::strtoull(self_user_id, nullptr, 10);
  if (uid == 0) return false;
  daveSessionReset(session);
  session->version = version;
  session->group_id = group_id;
  session->self_user_id = uid;
  session->sig_key = generate_p256_key();
  if (!session->sig_key || !export_p256_pub_65(session->sig_key.get(), session->sig_pub)) {
    return false;
  }
  session->regenerate_kem_slots();
  return true;
}

void daveSessionReset(DAVESessionHandle session) {
  if (!session) return;
  OPENSSL_cleanse(session->init_secret.data(), session->init_secret.size());
  OPENSSL_cleanse(session->exporter_secret.data(), session->exporter_secret.size());
  OPENSSL_cleanse(session->epoch_authenticator.data(), session->epoch_authenticator.size());
  session->in_established_group = false;
  session->epoch = 0;
  session->roster.clear();
  session->pending_proposals.clear();
  session->staged_own_commit.reset();
  session->transcript_hash.fill(0);
}

uint16_t daveSessionGetProtocolVersion(DAVESessionHandle session) {
  return session ? session->version : 0;
}

bool daveSessionSetExternalSender(DAVESessionHandle session,
                                  const uint8_t* external_sender,
                                  size_t length) {
  if (!session || !external_sender || length == 0 || session->in_established_group) return false;
  const uint8_t* p = external_sender;
  const uint8_t* end = p + length;
  std::vector<uint8_t> pub;
  uint16_t cred_type = 0;
  std::vector<uint8_t> ident;
  if (!read_vlbytes(p, end, pub) || pub.size() != kP256PubLen || pub[0] != 0x04) return false;
  if (!read_u16_be(p, end, cred_type) || cred_type != 1) return false;
  if (!read_vlbytes(p, end, ident) || p != end) return false;

  std::memcpy(session->external_sender_pub.data(), pub.data(), kP256PubLen);
  session->external_sender_raw.assign(external_sender, external_sender + length);
  session->has_external_sender = true;
  session->epoch = 0;
  session->roster = {session->self_member_node()};
  return true;
}

bool daveSessionGetMarshalledKeyPackage(DAVESessionHandle session,
                                        uint8_t** key_package,
                                        size_t* length) {
  if (!session || !key_package || !length || !session->sig_key) return false;
  session->regenerate_kem_slots();
  if (!session->in_established_group && session->has_external_sender) {
    session->roster = {session->self_member_node()};
  }

  std::vector<uint8_t> body;
  push_u16_be(body, kDaveProtocolVersionV1);
  push_u16_be(body, kMlsCiphersuiteId);
  push_u16_be(body, 1);
  std::vector<uint8_t> uid_be;
  push_u64_be(uid_be, session->self_user_id);
  write_vlbytes_vec(body, uid_be);
  write_vlbytes(body, session->sig_pub.data(), session->sig_pub.size());
  body.push_back(static_cast<uint8_t>(session->own_kem_slots.size()));
  for (const auto& kv : session->own_kem_slots) {
    body.push_back(kv.first);
    write_vlbytes(body, kv.second.first.data(), kv.second.first.size());
  }
  push_u64_be(body, 0ull);
  push_u64_be(body, 0xFFFFFFFFFFFFFFFFull);

  std::vector<uint8_t> tbs = {'M', 'L', 'S', ' ', '1', '.', '0', ' ', 'K', 'P'};
  tbs.insert(tbs.end(), body.begin(), body.end());
  std::array<uint8_t, kP256SigLen> sig{};
  if (!ecdsa_sign_fixed64(session->sig_key.get(), tbs.data(), tbs.size(), sig)) return false;

  std::vector<uint8_t> out;
  write_vlbytes_vec(out, body);
  write_vlbytes(out, sig.data(), sig.size());
  *key_package = alloc_copy_bytes(out, length);
  return *key_package != nullptr;
}

bool daveSessionProcessProposals(DAVESessionHandle session,
                                 uint8_t op_type,
                                 const uint8_t* proposals,
                                 size_t length,
                                 const uint64_t* recognized_user_ids,
                                 size_t recognized_user_ids_length,
                                 uint8_t** commit_bytes,
                                 size_t* commit_length,
                                 uint8_t** welcome_bytes,
                                 size_t* welcome_length) {
  if (!session || !session->has_external_sender || !proposals || length == 0) return false;
  *commit_bytes = nullptr;
  *commit_length = 0;
  *welcome_bytes = nullptr;
  *welcome_length = 0;

  std::set<uint64_t> allowed(recognized_user_ids,
                             recognized_user_ids + recognized_user_ids_length);
  if (length < 2 || proposals[0] != op_type) return false;
  const uint8_t* p = proposals + 1;
  const uint8_t* end = proposals + length;
  std::vector<uint8_t> inner;
  if (!read_vlbytes(p, end, inner) || p != end) return false;

  const uint8_t* ip = inner.data();
  const uint8_t* iend = ip + inner.size();
  if (op_type == 0) {
    while (ip < iend) {
      std::vector<uint8_t> prop_wire;
      if (!read_vlbytes(ip, iend, prop_wire)) return false;
      const uint8_t* pp = prop_wire.data();
      const uint8_t* pend = pp + prop_wire.size();
      std::vector<uint8_t> body;
      std::vector<uint8_t> sig;
      if (!read_vlbytes(pp, pend, body) || !read_vlbytes(pp, pend, sig) || pp != pend) return false;

      std::vector<uint8_t> tbs = {'M', 'L', 'S', ' ', '1', '.', '0', ' ', 'P', 'R', 'P'};
      tbs.insert(tbs.end(), body.begin(), body.end());
      if (!ecdsa_verify_fixed64(session->external_sender_pub.data(), tbs.data(),
                                tbs.size(), sig.data(), sig.size())) {
        return false;
      }

      const uint8_t* bp = body.data();
      const uint8_t* bend = bp + body.size();
      uint64_t gid = 0;
      uint64_t prop_epoch = 0;
      if (!read_u64_be(bp, bend, gid) || gid != session->group_id) return false;
      if (!read_u64_be(bp, bend, prop_epoch) || prop_epoch != session->epoch) return false;
      if (bp >= bend) return false;
      uint8_t ptype = *bp++;

      QueuedProposal qp{};
      auto ref_digest = sha256_digest(prop_wire.data(), prop_wire.size());
      std::memcpy(qp.ref.data(), ref_digest.data(), 16);

      if (ptype == 1) {
        std::vector<uint8_t> kp_raw;
        if (!read_vlbytes(bp, bend, kp_raw) || bp != bend) return false;
        if (!parse_and_verify_key_package(kp_raw, qp.added)) return false;
        if (!allowed.empty() && allowed.find(qp.added.user_id) == allowed.end()) return false;
        qp.is_add = true;
      } else if (ptype == 2) {
        if (!read_u64_be(bp, bend, qp.removed_user_id) || bp != bend) return false;
        qp.is_add = false;
      } else {
        return false;
      }
      session->pending_proposals.push_back(std::move(qp));
    }
  } else if (op_type == 1) {
    while (ip < iend) {
      std::vector<uint8_t> ref;
      if (!read_vlbytes(ip, iend, ref) || ref.size() != 16) return false;
      auto before = session->pending_proposals.size();
      session->pending_proposals.erase(
          std::remove_if(session->pending_proposals.begin(), session->pending_proposals.end(),
                         [&](const QueuedProposal& q) {
                           return std::memcmp(q.ref.data(), ref.data(), 16) == 0;
                         }),
          session->pending_proposals.end());
      if (session->pending_proposals.size() == before) return false;
    }
  } else {
    return false;
  }

  if (session->pending_proposals.empty()) {
    session->staged_own_commit.reset();
    return true;
  }

  std::vector<MemberNode> next_roster = session->roster;
  std::set<uint64_t> added_uids;
  for (const auto& qp : session->pending_proposals) {
    if (qp.is_add) {
      for (const auto& m : next_roster) {
        if (m.user_id == qp.added.user_id) return false;
      }
      next_roster.push_back(
          MemberNode{qp.added.user_id, qp.added.sig_pub, qp.added.kem_slots});
      added_uids.insert(qp.added.user_id);
    } else {
      next_roster.erase(
          std::remove_if(next_roster.begin(), next_roster.end(),
                         [&](const MemberNode& m) { return m.user_id == qp.removed_user_id; }),
          next_roster.end());
    }
  }
  if (next_roster.empty()) return false;
  std::sort(next_roster.begin(), next_roster.end(),
            [](const MemberNode& a, const MemberNode& b) { return a.user_id < b.user_id; });

  // Pop one KEM prekey slot per non-committer recipient BEFORE computing tree_hash!
  struct RecipientSlot {
    uint64_t uid;
    uint8_t slot_id;
    std::array<uint8_t, kP256PubLen> pub;
    bool is_welcome;
  };
  std::vector<RecipientSlot> recipients;
  for (auto& m : next_roster) {
    if (m.user_id == session->self_user_id) continue;
    if (m.kem_slots.empty()) return false;
    KemPubSlot chosen = m.kem_slots.front();
    m.kem_slots.erase(m.kem_slots.begin());
    recipients.push_back(RecipientSlot{
        m.user_id, chosen.slot_id, chosen.pub, added_uids.count(m.user_id) > 0});
  }

  std::vector<uint8_t> roster_bytes;
  serialize_roster(next_roster, roster_bytes);
  auto tree_hash = sha256_digest(roster_bytes.data(), roster_bytes.size());

  std::vector<uint8_t> trans_in(session->transcript_hash.begin(), session->transcript_hash.end());
  for (const auto& qp : session->pending_proposals) {
    trans_in.insert(trans_in.end(), qp.ref.begin(), qp.ref.end());
  }
  trans_in.insert(trans_in.end(), tree_hash.begin(), tree_hash.end());
  auto new_transcript = sha256_digest(trans_in.data(), trans_in.size());

  uint64_t new_epoch = session->epoch + 1;
  auto gc = build_group_context_bytes(session->group_id, new_epoch, tree_hash,
                                      new_transcript, session->external_sender_raw);

  std::array<uint8_t, 32> commit_secret{};
  RAND_bytes(commit_secret.data(), 32);
  auto derived = derive_epoch_from_commit(session->init_secret, commit_secret, gc, new_transcript);

  std::vector<uint8_t> commit_cts;
  std::vector<uint8_t> welcome_cts;
  uint16_t commit_ct_count = 0;
  uint16_t welcome_ct_count = 0;

  for (const auto& r : recipients) {
    std::vector<uint8_t> aad;
    push_u64_be(aad, session->group_id);
    push_u64_be(aad, new_epoch);
    push_u64_be(aad, r.uid);
    std::array<uint8_t, kP256PubLen> eph_pub{};
    std::vector<uint8_t> ct;
    if (!r.is_welcome) {
      std::vector<uint8_t> pt(commit_secret.begin(), commit_secret.end());
      if (!hpke_seal(r.pub.data(), aad, pt, eph_pub, ct)) return false;
      OPENSSL_cleanse(pt.data(), pt.size());
      push_u64_be(commit_cts, r.uid);
      commit_cts.push_back(r.slot_id);
      write_vlbytes(commit_cts, eph_pub.data(), eph_pub.size());
      write_vlbytes_vec(commit_cts, ct);
      ++commit_ct_count;
    } else {
      std::vector<uint8_t> pt(derived.joiner_secret.begin(), derived.joiner_secret.end());
      pt.insert(pt.end(), derived.init_secret.begin(), derived.init_secret.end());
      if (!hpke_seal(r.pub.data(), aad, pt, eph_pub, ct)) return false;
      OPENSSL_cleanse(pt.data(), pt.size());
      push_u64_be(welcome_cts, r.uid);
      welcome_cts.push_back(r.slot_id);
      write_vlbytes(welcome_cts, eph_pub.data(), eph_pub.size());
      write_vlbytes_vec(welcome_cts, ct);
      ++welcome_ct_count;
    }
  }
  OPENSSL_cleanse(commit_secret.data(), commit_secret.size());

  std::vector<uint8_t> commit_body;
  push_u64_be(commit_body, session->group_id);
  push_u64_be(commit_body, new_epoch);
  push_u64_be(commit_body, session->self_user_id);
  write_vlbytes(commit_body, new_transcript.data(), 32);
  write_vlbytes_vec(commit_body, roster_bytes);
  push_u16_be(commit_body, commit_ct_count);
  commit_body.insert(commit_body.end(), commit_cts.begin(), commit_cts.end());
  write_vlbytes(commit_body, derived.confirmation_tag.data(), 32);

  std::vector<uint8_t> c_tbs = {'M', 'L', 'S', ' ', '1', '.', '0', ' ', 'C', 'M', 'T'};
  c_tbs.insert(c_tbs.end(), commit_body.begin(), commit_body.end());
  std::array<uint8_t, kP256SigLen> c_sig{};
  if (!ecdsa_sign_fixed64(session->sig_key.get(), c_tbs.data(), c_tbs.size(), c_sig)) {
    return false;
  }

  std::vector<uint8_t> full_commit;
  write_vlbytes_vec(full_commit, commit_body);
  write_vlbytes(full_commit, c_sig.data(), c_sig.size());

  if (!added_uids.empty()) {
    std::vector<uint8_t> group_info_pt;
    push_u64_be(group_info_pt, session->group_id);
    push_u64_be(group_info_pt, new_epoch);
    push_u64_be(group_info_pt, session->self_user_id);
    write_vlbytes(group_info_pt, tree_hash.data(), 32);
    write_vlbytes(group_info_pt, new_transcript.data(), 32);
    write_vlbytes_vec(group_info_pt, session->external_sender_raw);
    write_vlbytes_vec(group_info_pt, roster_bytes);
    write_vlbytes(group_info_pt, derived.confirmation_tag.data(), 32);

    std::vector<uint8_t> w_tbs = {'M', 'L', 'S', ' ', '1', '.', '0', ' ', 'W', 'L', 'C'};
    w_tbs.insert(w_tbs.end(), group_info_pt.begin(), group_info_pt.end());
    std::array<uint8_t, kP256SigLen> w_sig{};
    ecdsa_sign_fixed64(session->sig_key.get(), w_tbs.data(), w_tbs.size(), w_sig);
    write_vlbytes(group_info_pt, w_sig.data(), w_sig.size());

    std::vector<uint8_t> enc_gi(group_info_pt.size() + kGcmTag16Len);
    aes128_gcm_seal_var_tag(derived.welcome_key.data(), derived.welcome_nonce.data(),
                            nullptr, 0, group_info_pt.data(), group_info_pt.size(),
                            enc_gi.data(), enc_gi.data() + group_info_pt.size(), kGcmTag16Len);

    std::vector<uint8_t> full_welcome;
    push_u16_be(full_welcome, kDaveProtocolVersionV1);
    push_u16_be(full_welcome, kMlsCiphersuiteId);
    push_u64_be(full_welcome, new_epoch);
    push_u16_be(full_welcome, welcome_ct_count);
    full_welcome.insert(full_welcome.end(), welcome_cts.begin(), welcome_cts.end());
    write_vlbytes_vec(full_welcome, enc_gi);
    *welcome_bytes = alloc_copy_bytes(full_welcome, welcome_length);
  }

  auto staged = std::make_unique<OpaqueDaveSession::StagedOwnCommit>();
  staged->raw_commit = full_commit;
  staged->new_epoch = new_epoch;
  staged->new_roster = std::move(next_roster);
  staged->derived = derived;
  staged->new_transcript = new_transcript;
  session->staged_own_commit = std::move(staged);

  *commit_bytes = alloc_copy_bytes(full_commit, commit_length);
  return true;
}

DAVECommitResultHandle daveSessionProcessCommit(DAVESessionHandle session,
                                                const uint8_t* commit,
                                                size_t length) {
  auto* res = new OpaqueDaveCommitResult();
  if (!session || !commit || length == 0 || !session->has_external_sender) {
    res->failed = true;
    return res;
  }

  if (session->staged_own_commit &&
      session->staged_own_commit->raw_commit.size() == length &&
      CRYPTO_memcmp(session->staged_own_commit->raw_commit.data(), commit, length) == 0) {
    auto staged = std::move(session->staged_own_commit);
    session->epoch = staged->new_epoch;
    session->roster = std::move(staged->new_roster);
    session->init_secret = staged->derived.init_secret;
    session->exporter_secret = staged->derived.exporter_secret;
    session->epoch_authenticator = staged->derived.epoch_authenticator;
    session->transcript_hash = staged->new_transcript;
    session->pending_proposals.clear();
    session->in_established_group = true;
    for (const auto& m : session->roster) res->roster.push_back(m.user_id);
    return res;
  }

  if (!session->in_established_group) {
    res->failed = true;
    return res;
  }

  const uint8_t* p = commit;
  const uint8_t* end = p + length;
  std::vector<uint8_t> body;
  std::vector<uint8_t> sig;
  if (!read_vlbytes(p, end, body) || !read_vlbytes(p, end, sig) || p != end) {
    res->failed = true;
    return res;
  }

  const uint8_t* bp = body.data();
  const uint8_t* bend = bp + body.size();
  uint64_t gid = 0;
  uint64_t new_epoch = 0;
  uint64_t sender_uid = 0;
  std::vector<uint8_t> new_trans_vec;
  std::vector<uint8_t> roster_bytes;
  uint16_t ct_count = 0;
  if (!read_u64_be(bp, bend, gid) || gid != session->group_id ||
      !read_u64_be(bp, bend, new_epoch) || new_epoch != session->epoch + 1 ||
      !read_u64_be(bp, bend, sender_uid) ||
      !read_vlbytes(bp, bend, new_trans_vec) || new_trans_vec.size() != 32 ||
      !read_vlbytes(bp, bend, roster_bytes) ||
      !read_u16_be(bp, bend, ct_count)) {
    res->failed = true;
    return res;
  }

  const MemberNode* sender_node = nullptr;
  for (const auto& m : session->roster) {
    if (m.user_id == sender_uid) sender_node = &m;
  }
  if (!sender_node) {
    res->failed = true;
    return res;
  }

  std::vector<uint8_t> c_tbs = {'M', 'L', 'S', ' ', '1', '.', '0', ' ', 'C', 'M', 'T'};
  c_tbs.insert(c_tbs.end(), body.begin(), body.end());
  if (!ecdsa_verify_fixed64(sender_node->sig_pub.data(), c_tbs.data(), c_tbs.size(),
                            sig.data(), sig.size())) {
    res->failed = true;
    return res;
  }

  bool found_mine = false;
  uint8_t my_slot = 0;
  std::vector<uint8_t> my_eph;
  std::vector<uint8_t> my_ct;
  for (uint16_t i = 0; i < ct_count; ++i) {
    uint64_t ruid = 0;
    std::vector<uint8_t> ep;
    std::vector<uint8_t> ct;
    if (!read_u64_be(bp, bend, ruid) || bp >= bend) {
      res->failed = true;
      return res;
    }
    uint8_t sid = *bp++;
    if (!read_vlbytes(bp, bend, ep) || ep.size() != kP256PubLen ||
        !read_vlbytes(bp, bend, ct)) {
      res->failed = true;
      return res;
    }
    if (ruid == session->self_user_id) {
      found_mine = true;
      my_slot = sid;
      my_eph = std::move(ep);
      my_ct = std::move(ct);
    }
  }
  std::vector<uint8_t> recv_conf_tag;
  if (!read_vlbytes(bp, bend, recv_conf_tag) || recv_conf_tag.size() != 32 || bp != bend ||
      !found_mine) {
    res->failed = true;
    return res;
  }

  auto slot_it = session->own_kem_slots.find(my_slot);
  if (slot_it == session->own_kem_slots.end()) {
    res->failed = true;
    return res;
  }
  std::vector<uint8_t> aad;
  push_u64_be(aad, session->group_id);
  push_u64_be(aad, new_epoch);
  push_u64_be(aad, session->self_user_id);
  std::vector<uint8_t> commit_secret_vec;
  bool dec_ok = hpke_open(slot_it->second.second.get(), slot_it->second.first,
                          my_eph.data(), aad, my_ct, commit_secret_vec);
  session->own_kem_slots.erase(slot_it);
  if (!dec_ok || commit_secret_vec.size() != 32) {
    res->failed = true;
    return res;
  }

  const uint8_t* rp = roster_bytes.data();
  std::vector<MemberNode> new_roster;
  if (!deserialize_roster(rp, rp + roster_bytes.size(), new_roster) ||
      rp != roster_bytes.data() + roster_bytes.size()) {
    res->failed = true;
    return res;
  }

  auto tree_hash = sha256_digest(roster_bytes.data(), roster_bytes.size());
  std::array<uint8_t, 32> new_transcript{};
  std::memcpy(new_transcript.data(), new_trans_vec.data(), 32);
  auto gc = build_group_context_bytes(session->group_id, new_epoch, tree_hash,
                                      new_transcript, session->external_sender_raw);
  std::array<uint8_t, 32> cs{};
  std::memcpy(cs.data(), commit_secret_vec.data(), 32);
  OPENSSL_cleanse(commit_secret_vec.data(), commit_secret_vec.size());

  auto derived = derive_epoch_from_commit(session->init_secret, cs, gc, new_transcript);
  OPENSSL_cleanse(cs.data(), cs.size());
  if (CRYPTO_memcmp(derived.confirmation_tag.data(), recv_conf_tag.data(), 32) != 0) {
    res->failed = true;
    return res;
  }

  session->epoch = new_epoch;
  session->roster = std::move(new_roster);
  session->init_secret = derived.init_secret;
  session->exporter_secret = derived.exporter_secret;
  session->epoch_authenticator = derived.epoch_authenticator;
  session->transcript_hash = new_transcript;
  session->pending_proposals.clear();
  session->staged_own_commit.reset();
  for (const auto& m : session->roster) res->roster.push_back(m.user_id);
  return res;
}

DAVEWelcomeResultHandle daveSessionProcessWelcome(
    DAVESessionHandle session,
    const uint8_t* welcome,
    size_t length,
    const uint64_t* recognized_user_ids,
    size_t recognized_user_ids_length) {
  auto* res = new OpaqueDaveWelcomeResult();
  if (!session || !welcome || length < 14 || !session->has_external_sender ||
      session->in_established_group) {
    res->failed = true;
    return res;
  }

  std::set<uint64_t> allowed(recognized_user_ids,
                             recognized_user_ids + recognized_user_ids_length);
  const uint8_t* p = welcome;
  const uint8_t* end = p + length;
  uint16_t ver = 0;
  uint16_t suite = 0;
  uint64_t w_epoch = 0;
  uint16_t ct_count = 0;
  if (!read_u16_be(p, end, ver) || ver != kDaveProtocolVersionV1 ||
      !read_u16_be(p, end, suite) || suite != kMlsCiphersuiteId ||
      !read_u64_be(p, end, w_epoch) ||
      !read_u16_be(p, end, ct_count) || ct_count == 0) {
    res->failed = true;
    return res;
  }

  bool found_mine = false;
  uint8_t my_slot = 0;
  std::vector<uint8_t> my_eph;
  std::vector<uint8_t> my_ct;
  for (uint16_t i = 0; i < ct_count; ++i) {
    uint64_t ruid = 0;
    std::vector<uint8_t> ep;
    std::vector<uint8_t> ct;
    if (!read_u64_be(p, end, ruid) || p >= end) {
      res->failed = true;
      return res;
    }
    uint8_t sid = *p++;
    if (!read_vlbytes(p, end, ep) || ep.size() != kP256PubLen ||
        !read_vlbytes(p, end, ct)) {
      res->failed = true;
      return res;
    }
    if (ruid == session->self_user_id) {
      found_mine = true;
      my_slot = sid;
      my_eph = std::move(ep);
      my_ct = std::move(ct);
    }
  }

  std::vector<uint8_t> enc_gi;
  if (!read_vlbytes(p, end, enc_gi) || enc_gi.size() < kGcmTag16Len || p != end || !found_mine) {
    res->failed = true;
    return res;
  }

  auto slot_it = session->own_kem_slots.find(my_slot);
  if (slot_it == session->own_kem_slots.end()) {
    res->failed = true;
    return res;
  }

  std::vector<uint8_t> aad;
  push_u64_be(aad, session->group_id);
  push_u64_be(aad, w_epoch);
  push_u64_be(aad, session->self_user_id);

  std::vector<uint8_t> kp_pt;
  bool dec_ok = hpke_open(slot_it->second.second.get(), slot_it->second.first,
                          my_eph.data(), aad, my_ct, kp_pt);
  session->own_kem_slots.erase(slot_it);
  if (!dec_ok || kp_pt.size() != 64) {
    res->failed = true;
    return res;
  }

  std::array<uint8_t, 32> joiner{};
  std::memcpy(joiner.data(), kp_pt.data(), 32);
  OPENSSL_cleanse(kp_pt.data(), kp_pt.size());

  std::array<uint8_t, 32> zero32{};
  auto w_keys = derive_epoch_from_joiner(joiner, {}, zero32);
  size_t gi_len = enc_gi.size() - kGcmTag16Len;
  std::vector<uint8_t> gi_full(gi_len);
  if (!aes128_gcm_open_var_tag(w_keys.welcome_key.data(), w_keys.welcome_nonce.data(),
                               nullptr, 0, enc_gi.data(), gi_len,
                               enc_gi.data() + gi_len, kGcmTag16Len, gi_full.data())) {
    OPENSSL_cleanse(joiner.data(), joiner.size());
    res->failed = true;
    return res;
  }

  const uint8_t* gp = gi_full.data();
  const uint8_t* gend = gp + gi_full.size();
  uint64_t gid = 0;
  uint64_t ep = 0;
  uint64_t sender_uid = 0;
  std::vector<uint8_t> th_v;
  std::vector<uint8_t> tr_v;
  std::vector<uint8_t> joined_ext;
  std::vector<uint8_t> roster_b;
  std::vector<uint8_t> conf_tag;
  if (!read_u64_be(gp, gend, gid) || gid != session->group_id ||
      !read_u64_be(gp, gend, ep) || ep != w_epoch ||
      !read_u64_be(gp, gend, sender_uid) ||
      !read_vlbytes(gp, gend, th_v) || th_v.size() != 32 ||
      !read_vlbytes(gp, gend, tr_v) || tr_v.size() != 32 ||
      !read_vlbytes(gp, gend, joined_ext) || joined_ext != session->external_sender_raw ||
      !read_vlbytes(gp, gend, roster_b) ||
      !read_vlbytes(gp, gend, conf_tag) || conf_tag.size() != 32) {
    OPENSSL_cleanse(joiner.data(), joiner.size());
    res->failed = true;
    return res;
  }
  size_t signed_len = static_cast<size_t>(gp - gi_full.data());
  std::vector<uint8_t> w_sig;
  if (!read_vlbytes(gp, gend, w_sig) || gp != gend) {
    OPENSSL_cleanse(joiner.data(), joiner.size());
    res->failed = true;
    return res;
  }

  const uint8_t* rp = roster_b.data();
  std::vector<MemberNode> roster;
  if (!deserialize_roster(rp, rp + roster_b.size(), roster) || rp != roster_b.data() + roster_b.size()) {
    OPENSSL_cleanse(joiner.data(), joiner.size());
    res->failed = true;
    return res;
  }

  const MemberNode* sender_node = nullptr;
  bool has_self = false;
  for (const auto& m : roster) {
    if (m.user_id == sender_uid) sender_node = &m;
    if (m.user_id == session->self_user_id) has_self = true;
    if (!allowed.empty() && allowed.find(m.user_id) == allowed.end()) {
      OPENSSL_cleanse(joiner.data(), joiner.size());
      res->failed = true;
      return res;
    }
  }
  if (!sender_node || !has_self) {
    OPENSSL_cleanse(joiner.data(), joiner.size());
    res->failed = true;
    return res;
  }

  std::vector<uint8_t> w_tbs = {'M', 'L', 'S', ' ', '1', '.', '0', ' ', 'W', 'L', 'C'};
  w_tbs.insert(w_tbs.end(), gi_full.begin(), gi_full.begin() + signed_len);
  if (!ecdsa_verify_fixed64(sender_node->sig_pub.data(), w_tbs.data(), w_tbs.size(),
                            w_sig.data(), w_sig.size())) {
    OPENSSL_cleanse(joiner.data(), joiner.size());
    res->failed = true;
    return res;
  }

  auto computed_th = sha256_digest(roster_b.data(), roster_b.size());
  if (CRYPTO_memcmp(computed_th.data(), th_v.data(), 32) != 0) {
    OPENSSL_cleanse(joiner.data(), joiner.size());
    res->failed = true;
    return res;
  }

  std::array<uint8_t, 32> transcript{};
  std::memcpy(transcript.data(), tr_v.data(), 32);
  auto gc = build_group_context_bytes(gid, ep, computed_th, transcript, session->external_sender_raw);
  auto derived = derive_epoch_from_joiner(joiner, gc, transcript);
  OPENSSL_cleanse(joiner.data(), joiner.size());

  if (CRYPTO_memcmp(derived.confirmation_tag.data(), conf_tag.data(), 32) != 0) {
    res->failed = true;
    return res;
  }

  session->epoch = ep;
  session->roster = std::move(roster);
  session->init_secret = derived.init_secret;
  session->exporter_secret = derived.exporter_secret;
  session->epoch_authenticator = derived.epoch_authenticator;
  session->transcript_hash = transcript;
  session->pending_proposals.clear();
  session->staged_own_commit.reset();
  session->in_established_group = true;
  for (const auto& m : session->roster) res->roster.push_back(m.user_id);
  return res;
}

DAVEKeyRatchetHandle daveSessionGetKeyRatchet(DAVESessionHandle session,
                                              const char* user_id) {
  if (!session || !session->in_established_group || !user_id) return nullptr;
  uint64_t uid = std::strtoull(user_id, nullptr, 10);
  bool found = false;
  for (const auto& m : session->roster) {
    if (m.user_id == uid) found = true;
  }
  if (!found) return nullptr;
  auto* kr = new OpaqueDaveKeyRatchet();
  kr->base_secret = export_user_media_base_secret(session->exporter_secret, uid);
  return kr;
}

bool daveSessionGetLastEpochAuthenticator(DAVESessionHandle session,
                                          uint8_t** authenticator,
                                          size_t* length) {
  if (!session || !session->in_established_group || !authenticator || !length) return false;
  std::vector<uint8_t> v(session->epoch_authenticator.begin(),
                         session->epoch_authenticator.end());
  *authenticator = alloc_copy_bytes(v, length);
  return *authenticator != nullptr;
}

bool daveSessionGetEpoch(DAVESessionHandle session, uint64_t* out_epoch) {
  if (!session || !session->has_external_sender || !out_epoch) return false;
  *out_epoch = session->epoch;
  return true;
}

bool daveSessionGetPairwiseFingerprint(DAVESessionHandle session,
                                       uint16_t version,
                                       const char* user_id,
                                       uint8_t** fingerprint,
                                       size_t* length) {
  if (!session || !session->in_established_group || version != 0 || !user_id ||
      !fingerprint || !length) {
    return false;
  }
  uint64_t peer_uid = std::strtoull(user_id, nullptr, 10);
  const MemberNode* peer = nullptr;
  for (const auto& m : session->roster) {
    if (m.user_id == peer_uid) peer = &m;
  }
  if (!peer) return false;

  auto build_fp = [&](uint64_t uid, const std::array<uint8_t, kP256PubLen>& pk) {
    std::vector<uint8_t> b;
    push_u16_be(b, version);
    b.insert(b.end(), pk.begin(), pk.end());
    push_u64_be(b, uid);
    return b;
  };
  std::vector<uint8_t> a = build_fp(session->self_user_id, session->sig_pub);
  std::vector<uint8_t> b = build_fp(peer_uid, peer->sig_pub);
  if (b < a) std::swap(a, b);
  std::vector<uint8_t> ikm = a;
  ikm.insert(ikm.end(), b.begin(), b.end());

  auto prk = hkdf_extract_sha256(kFingerprintSalt, 16, ikm.data(), ikm.size());
  static const char kInfo[] = "Discord DAVE Pairwise v0";
  std::vector<uint8_t> out(64, 0);
  hkdf_expand_sha256(prk.data(), prk.size(), reinterpret_cast<const uint8_t*>(kInfo),
                     sizeof(kInfo) - 1, out.data(), out.size());
  OPENSSL_cleanse(prk.data(), prk.size());
  *fingerprint = alloc_copy_bytes(out, length);
  return *fingerprint != nullptr;
}

bool daveCommitResultIsFailed(DAVECommitResultHandle result) {
  return !result || result->failed;
}

bool daveCommitResultIsIgnored(DAVECommitResultHandle result) {
  return result && result->ignored;
}

void daveCommitResultGetRosterMemberIds(DAVECommitResultHandle result,
                                        uint64_t** roster_ids,
                                        size_t* roster_length) {
  if (!result || !roster_ids || !roster_length) return;
  *roster_ids = alloc_copy_u64s(result->roster, roster_length);
}

void daveCommitResultDestroy(DAVECommitResultHandle result) { delete result; }

bool daveWelcomeResultIsFailed(DAVEWelcomeResultHandle result) {
  return !result || result->failed;
}

void daveWelcomeResultGetRosterMemberIds(DAVEWelcomeResultHandle result,
                                         uint64_t** roster_ids,
                                         size_t* roster_length) {
  if (!result || !roster_ids || !roster_length) return;
  *roster_ids = alloc_copy_u64s(result->roster, roster_length);
}

void daveWelcomeResultDestroy(DAVEWelcomeResultHandle result) { delete result; }

void daveKeyRatchetDestroy(DAVEKeyRatchetHandle key_ratchet) { delete key_ratchet; }

bool daveKeyRatchetDeriveKeyAndNonceForTest(const uint8_t* base_secret,
                                            size_t base_secret_len,
                                            uint32_t generation,
                                            uint8_t* out_key16,
                                            uint8_t* out_nonce12) {
  if (!base_secret || base_secret_len != 16 || !out_key16 || !out_nonce12) return false;
  std::array<uint8_t, 16> base{};
  std::memcpy(base.data(), base_secret, 16);
  HashRatchetImpl r(base);
  std::array<uint8_t, 16> k{};
  std::array<uint8_t, 12> n{};
  if (!r.get(generation, k, n)) return false;
  std::memcpy(out_key16, k.data(), 16);
  std::memcpy(out_nonce12, n.data(), 12);
  OPENSSL_cleanse(k.data(), k.size());
  return true;
}

bool daveAes128GcmTruncated8EncryptForTest(const uint8_t* key16,
                                           const uint8_t* nonce12,
                                           const uint8_t* aad,
                                           size_t aad_len,
                                           const uint8_t* plaintext,
                                           size_t plaintext_len,
                                           uint8_t* ciphertext_out,
                                           uint8_t* tag8_out) {
  return aes128_gcm_seal_var_tag(key16, nonce12, aad, aad_len, plaintext,
                                 plaintext_len, ciphertext_out, tag8_out, kDaveTag8Len);
}

DAVEEncryptorHandle daveEncryptorCreate(void) { return new OpaqueDaveEncryptor(); }

void daveEncryptorDestroy(DAVEEncryptorHandle encryptor) { delete encryptor; }

void daveEncryptorSetKeyRatchet(DAVEEncryptorHandle encryptor,
                                DAVEKeyRatchetHandle key_ratchet) {
  if (!encryptor || !key_ratchet) return;
  OPENSSL_cleanse(encryptor->current_key.data(), encryptor->current_key.size());
  encryptor->ratchet = std::make_unique<HashRatchetImpl>(key_ratchet->base_secret);
  encryptor->current_gen = 0;
  encryptor->truncated_nonce = 0;
  encryptor->has_key = false;
}

void daveEncryptorSetPassthroughMode(DAVEEncryptorHandle encryptor, bool passthrough) {
  if (encryptor) encryptor->passthrough = passthrough;
}

size_t daveEncryptorGetMaxCiphertextByteSize(DAVEEncryptorHandle,
                                             int32_t,
                                             size_t frame_size) {
  return frame_size + 64;
}

int32_t daveEncryptorEncrypt(DAVEEncryptorHandle encryptor,
                             int32_t media_type,
                             int32_t codec,
                             const uint8_t* frame,
                             size_t frame_length,
                             uint8_t* encrypted_frame,
                             size_t encrypted_frame_capacity,
                             size_t* bytes_written) {
  if (!encryptor || !encrypted_frame || !bytes_written || (!frame && frame_length > 0)) {
    return DAVE_ENCRYPTOR_RESULT_CODE_ENCRYPTION_FAILURE;
  }
  if (media_type == DAVE_MEDIA_TYPE_AUDIO && frame_length == 3 &&
      std::memcmp(frame, kOpusSilence, 3) == 0) {
    if (encrypted_frame_capacity < 3) return DAVE_ENCRYPTOR_RESULT_CODE_ENCRYPTION_FAILURE;
    std::memcpy(encrypted_frame, kOpusSilence, 3);
    *bytes_written = 3;
    return DAVE_ENCRYPTOR_RESULT_CODE_SUCCESS;
  }
  if (!encryptor->ratchet) {
    if (encryptor->passthrough && encrypted_frame_capacity >= frame_length) {
      if (frame_length > 0) std::memcpy(encrypted_frame, frame, frame_length);
      *bytes_written = frame_length;
      return DAVE_ENCRYPTOR_RESULT_CODE_SUCCESS;
    }
    return DAVE_ENCRYPTOR_RESULT_CODE_MISSING_KEY_RATCHET;
  }

  std::vector<Range> ranges;
  if (codec == DAVE_CODEC_H264) {
    ranges = h264_unencrypted_ranges(frame, frame_length);
  }
  std::vector<uint8_t> aad;
  std::vector<uint8_t> plain;
  size_t cursor = 0;
  for (const auto& r : ranges) {
    if (r.offset > cursor) plain.insert(plain.end(), frame + cursor, frame + r.offset);
    aad.insert(aad.end(), frame + r.offset, frame + r.offset + r.size);
    cursor = r.offset + r.size;
  }
  if (cursor < frame_length) plain.insert(plain.end(), frame + cursor, frame + frame_length);

  for (int attempt = 0; attempt < 10; ++attempt) {
    encryptor->truncated_nonce += 1;
    uint32_t nonce32 = encryptor->truncated_nonce;
    uint32_t next_gen = compute_wrapped_generation(encryptor->current_gen, nonce32 >> 24);
    if (next_gen != encryptor->current_gen || !encryptor->has_key) {
      std::array<uint8_t, 16> key{};
      std::array<uint8_t, 12> dummy_n{};
      if (!encryptor->ratchet->get(next_gen, key, dummy_n)) {
        return DAVE_ENCRYPTOR_RESULT_CODE_MISSING_CRYPTOR;
      }
      OPENSSL_cleanse(encryptor->current_key.data(), encryptor->current_key.size());
      encryptor->current_key = key;
      OPENSSL_cleanse(key.data(), key.size());
      encryptor->current_gen = next_gen;
      encryptor->has_key = true;
      if (next_gen > 0) encryptor->ratchet->prune_before(next_gen);
    }

    uint8_t nonce12[12] = {0};
    for (int i = 0; i < 4; ++i) nonce12[8 + i] = static_cast<uint8_t>((nonce32 >> (8 * i)) & 0xFF);

    std::vector<uint8_t> ct(plain.size());
    uint8_t tag8[kDaveTag8Len] = {0};
    if (!aes128_gcm_seal_var_tag(encryptor->current_key.data(), nonce12,
                                 aad.data(), aad.size(), plain.data(), plain.size(),
                                 ct.data(), tag8, kDaveTag8Len)) {
      return DAVE_ENCRYPTOR_RESULT_CODE_ENCRYPTION_FAILURE;
    }

    // Reconstruct payload [clear ranges + ct]
    if (encrypted_frame_capacity < frame_length + 32) {
      return DAVE_ENCRYPTOR_RESULT_CODE_ENCRYPTION_FAILURE;
    }
    size_t out_pos = 0;
    size_t aad_pos = 0;
    size_t ct_pos = 0;
    for (const auto& r : ranges) {
      if (r.offset > out_pos) {
        size_t n = r.offset - out_pos;
        std::memcpy(encrypted_frame + out_pos, ct.data() + ct_pos, n);
        ct_pos += n;
        out_pos += n;
      }
      std::memcpy(encrypted_frame + out_pos, aad.data() + aad_pos, r.size);
      aad_pos += r.size;
      out_pos += r.size;
    }
    if (ct_pos < ct.size()) {
      size_t n = ct.size() - ct_pos;
      std::memcpy(encrypted_frame + out_pos, ct.data() + ct_pos, n);
      out_pos += n;
    }

    if (ciphertext_has_h26x_start_code(ranges, encrypted_frame, out_pos)) {
      continue;
    }

    size_t supp_start = out_pos;
    std::memcpy(encrypted_frame + out_pos, tag8, kDaveTag8Len);
    out_pos += kDaveTag8Len;
    out_pos += write_leb128(nonce32, encrypted_frame + out_pos);
    for (const auto& r : ranges) {
      out_pos += write_leb128(r.offset, encrypted_frame + out_pos);
      out_pos += write_leb128(r.size, encrypted_frame + out_pos);
    }
    size_t supp_total = (out_pos - supp_start) + 1 + 2;
    if (supp_total > 255) return DAVE_ENCRYPTOR_RESULT_CODE_ENCRYPTION_FAILURE;
    encrypted_frame[out_pos++] = static_cast<uint8_t>(supp_total);
    encrypted_frame[out_pos++] = kMarker0;
    encrypted_frame[out_pos++] = kMarker1;
    *bytes_written = out_pos;
    return DAVE_ENCRYPTOR_RESULT_CODE_SUCCESS;
  }
  return DAVE_ENCRYPTOR_RESULT_CODE_TOO_MANY_ATTEMPTS;
}

DAVEDecryptorHandle daveDecryptorCreate(void) { return new OpaqueDaveDecryptor(); }

void daveDecryptorDestroy(DAVEDecryptorHandle decryptor) { delete decryptor; }

void daveDecryptorTransitionToKeyRatchet(DAVEDecryptorHandle decryptor,
                                         DAVEKeyRatchetHandle key_ratchet) {
  if (!decryptor || !key_ratchet) return;
  if (decryptor->managers.size() >= 2) decryptor->managers.pop_front();
  CipherManagerEntry entry{};
  entry.ratchet = std::make_unique<HashRatchetImpl>(key_ratchet->base_secret);
  decryptor->managers.push_back(std::move(entry));
}

void daveDecryptorTransitionToPassthroughMode(DAVEDecryptorHandle decryptor,
                                              bool passthrough) {
  if (decryptor) decryptor->passthrough = passthrough;
}

size_t daveDecryptorGetMaxPlaintextByteSize(DAVEDecryptorHandle,
                                            int32_t,
                                            size_t encrypted_frame_size) {
  return encrypted_frame_size;
}

int32_t daveDecryptorDecrypt(DAVEDecryptorHandle decryptor,
                             int32_t media_type,
                             const uint8_t* encrypted_frame,
                             size_t encrypted_frame_length,
                             uint8_t* frame,
                             size_t frame_capacity,
                             size_t* bytes_written) {
  if (!decryptor || !encrypted_frame || !frame || !bytes_written) {
    return DAVE_DECRYPTOR_RESULT_CODE_DECRYPTION_FAILURE;
  }
  if (media_type == DAVE_MEDIA_TYPE_AUDIO && encrypted_frame_length == 3 &&
      std::memcmp(encrypted_frame, kOpusSilence, 3) == 0) {
    if (frame_capacity < 3) return DAVE_DECRYPTOR_RESULT_CODE_DECRYPTION_FAILURE;
    std::memcpy(frame, kOpusSilence, 3);
    *bytes_written = 3;
    return DAVE_DECRYPTOR_RESULT_CODE_SUCCESS;
  }
  if (encrypted_frame_length < 12 ||
      encrypted_frame[encrypted_frame_length - 2] != kMarker0 ||
      encrypted_frame[encrypted_frame_length - 1] != kMarker1) {
    if (decryptor->passthrough && frame_capacity >= encrypted_frame_length) {
      std::memcpy(frame, encrypted_frame, encrypted_frame_length);
      *bytes_written = encrypted_frame_length;
      return DAVE_DECRYPTOR_RESULT_CODE_SUCCESS;
    }
    return DAVE_DECRYPTOR_RESULT_CODE_DECRYPTION_FAILURE;
  }

  size_t supp_size = encrypted_frame[encrypted_frame_length - 3];
  if (supp_size < 12 || supp_size > encrypted_frame_length) {
    return DAVE_DECRYPTOR_RESULT_CODE_DECRYPTION_FAILURE;
  }
  size_t payload_len = encrypted_frame_length - supp_size;
  const uint8_t* supp = encrypted_frame + payload_len;
  const uint8_t* tag8 = supp;
  const uint8_t* meta = supp + kDaveTag8Len;
  size_t meta_len = supp_size - kDaveTag8Len - 3;

  uint32_t nonce32 = 0;
  size_t used = 0;
  if (!read_leb128(meta, meta_len, nonce32, used)) {
    return DAVE_DECRYPTOR_RESULT_CODE_INVALID_NONCE;
  }
  size_t rem_pos = used;
  std::vector<Range> ranges;
  size_t prev_end = 0;
  while (rem_pos < meta_len) {
    uint32_t off = 0;
    uint32_t sz = 0;
    size_t u1 = 0;
    size_t u2 = 0;
    if (!read_leb128(meta + rem_pos, meta_len - rem_pos, off, u1)) {
      return DAVE_DECRYPTOR_RESULT_CODE_DECRYPTION_FAILURE;
    }
    rem_pos += u1;
    if (!read_leb128(meta + rem_pos, meta_len - rem_pos, sz, u2)) {
      return DAVE_DECRYPTOR_RESULT_CODE_DECRYPTION_FAILURE;
    }
    rem_pos += u2;
    if (off < prev_end || static_cast<size_t>(off) + static_cast<size_t>(sz) > payload_len) {
      return DAVE_DECRYPTOR_RESULT_CODE_DECRYPTION_FAILURE;
    }
    prev_end = static_cast<size_t>(off) + static_cast<size_t>(sz);
    ranges.push_back(Range{off, sz});
  }

  std::vector<uint8_t> aad;
  std::vector<uint8_t> ct;
  size_t cursor = 0;
  for (const auto& r : ranges) {
    if (r.offset > cursor) ct.insert(ct.end(), encrypted_frame + cursor, encrypted_frame + r.offset);
    aad.insert(aad.end(), encrypted_frame + r.offset, encrypted_frame + r.offset + r.size);
    cursor = r.offset + r.size;
  }
  if (cursor < payload_len) ct.insert(ct.end(), encrypted_frame + cursor, encrypted_frame + payload_len);

  uint8_t nonce12[12] = {0};
  for (int i = 0; i < 4; ++i) nonce12[8 + i] = static_cast<uint8_t>((nonce32 >> (8 * i)) & 0xFF);
  uint32_t gen_lsb = nonce32 >> 24;

  for (auto it = decryptor->managers.rbegin(); it != decryptor->managers.rend(); ++it) {
    uint32_t gen = compute_wrapped_generation(it->oldest_gen, gen_lsb);
    uint64_t big_nonce = compute_wrapped_big_nonce(gen, nonce32);
    if (!it->replay.can_accept(big_nonce)) continue;

    std::array<uint8_t, 16> key{};
    std::array<uint8_t, 12> dummy_n{};
    if (!it->ratchet->get(gen, key, dummy_n)) continue;

    std::vector<uint8_t> plain(ct.size());
    bool ok = aes128_gcm_open_var_tag(key.data(), nonce12, aad.data(), aad.size(),
                                      ct.data(), ct.size(), tag8, kDaveTag8Len, plain.data());
    OPENSSL_cleanse(key.data(), key.size());
    if (ok) {
      it->replay.accept(big_nonce);
      if (gen > it->newest_gen) {
        it->newest_gen = gen;
        if (gen > 4) {
          it->oldest_gen = std::max(it->oldest_gen, gen - 4);
          it->ratchet->prune_before(it->oldest_gen);
        }
      }
      if (frame_capacity < payload_len) return DAVE_DECRYPTOR_RESULT_CODE_DECRYPTION_FAILURE;
      size_t out_pos = 0;
      size_t aad_pos = 0;
      size_t pt_pos = 0;
      for (const auto& r : ranges) {
        if (r.offset > out_pos) {
          size_t n = r.offset - out_pos;
          std::memcpy(frame + out_pos, plain.data() + pt_pos, n);
          pt_pos += n;
          out_pos += n;
        }
        std::memcpy(frame + out_pos, aad.data() + aad_pos, r.size);
        aad_pos += r.size;
        out_pos += r.size;
      }
      if (pt_pos < plain.size()) {
        size_t n = plain.size() - pt_pos;
        std::memcpy(frame + out_pos, plain.data() + pt_pos, n);
        out_pos += n;
      }
      *bytes_written = out_pos;
      return DAVE_DECRYPTOR_RESULT_CODE_SUCCESS;
    }
  }
  return DAVE_DECRYPTOR_RESULT_CODE_DECRYPTION_FAILURE;
}

bool daveTestCreateExternalSender(const uint8_t* label,
                                  size_t label_len,
                                  uint8_t** out_package,
                                  size_t* out_package_len,
                                  void** out_private_handle) {
  auto* priv = new ExternalSenderPrivate();
  priv->sig_key = generate_p256_key();
  if (!priv->sig_key || !export_p256_pub_65(priv->sig_key.get(), priv->sig_pub)) {
    delete priv;
    return false;
  }
  std::vector<uint8_t> pkg;
  write_vlbytes(pkg, priv->sig_pub.data(), priv->sig_pub.size());
  push_u16_be(pkg, 1);
  write_vlbytes(pkg, label, label_len);
  *out_package = alloc_copy_bytes(pkg, out_package_len);
  *out_private_handle = priv;
  return *out_package != nullptr;
}

void daveTestDestroyExternalSender(void* private_handle) {
  delete static_cast<ExternalSenderPrivate*>(private_handle);
}

static bool sign_test_proposal_body(ExternalSenderPrivate* priv,
                                    const std::vector<uint8_t>& body,
                                    uint8_t** out_payload,
                                    size_t* out_payload_len) {
  std::vector<uint8_t> tbs = {'M', 'L', 'S', ' ', '1', '.', '0', ' ', 'P', 'R', 'P'};
  tbs.insert(tbs.end(), body.begin(), body.end());
  std::array<uint8_t, kP256SigLen> sig{};
  if (!ecdsa_sign_fixed64(priv->sig_key.get(), tbs.data(), tbs.size(), sig)) return false;

  std::vector<uint8_t> prop_wire;
  write_vlbytes_vec(prop_wire, body);
  write_vlbytes(prop_wire, sig.data(), sig.size());

  std::vector<uint8_t> inner;
  write_vlbytes_vec(inner, prop_wire);

  std::vector<uint8_t> payload = {0u};
  write_vlbytes_vec(payload, inner);
  *out_payload = alloc_copy_bytes(payload, out_payload_len);
  return *out_payload != nullptr;
}

bool daveTestSignAddProposal(void* private_handle,
                             uint64_t group_id,
                             uint64_t epoch,
                             const uint8_t* key_package,
                             size_t key_package_len,
                             uint8_t** out_payload,
                             size_t* out_payload_len) {
  auto* priv = static_cast<ExternalSenderPrivate*>(private_handle);
  if (!priv || !key_package || key_package_len == 0) return false;
  std::vector<uint8_t> body;
  push_u64_be(body, group_id);
  push_u64_be(body, epoch);
  body.push_back(1);
  write_vlbytes(body, key_package, key_package_len);
  return sign_test_proposal_body(priv, body, out_payload, out_payload_len);
}

bool daveTestSignRemoveProposal(void* private_handle,
                                uint64_t group_id,
                                uint64_t epoch,
                                uint64_t removed_user_id,
                                uint8_t** out_payload,
                                size_t* out_payload_len) {
  auto* priv = static_cast<ExternalSenderPrivate*>(private_handle);
  if (!priv) return false;
  std::vector<uint8_t> body;
  push_u64_be(body, group_id);
  push_u64_be(body, epoch);
  body.push_back(2);
  push_u64_be(body, removed_user_id);
  return sign_test_proposal_body(priv, body, out_payload, out_payload_len);
}

}  // extern "C"


