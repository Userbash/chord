#ifndef DISCORD_DAVE_C_API_H_
#define DISCORD_DAVE_C_API_H_

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct OpaqueDaveSession* DAVESessionHandle;
typedef struct OpaqueDaveCommitResult* DAVECommitResultHandle;
typedef struct OpaqueDaveWelcomeResult* DAVEWelcomeResultHandle;
typedef struct OpaqueDaveKeyRatchet* DAVEKeyRatchetHandle;
typedef struct OpaqueDaveEncryptor* DAVEEncryptorHandle;
typedef struct OpaqueDaveDecryptor* DAVEDecryptorHandle;

typedef enum DAVEMediaType {
  DAVE_MEDIA_TYPE_AUDIO = 0,
  DAVE_MEDIA_TYPE_VIDEO = 1,
} DAVEMediaType;

typedef enum DAVECodec {
  DAVE_CODEC_UNKNOWN = 0,
  DAVE_CODEC_OPUS = 1,
  DAVE_CODEC_VP8 = 2,
  DAVE_CODEC_VP9 = 3,
  DAVE_CODEC_H264 = 4,
  DAVE_CODEC_H265 = 5,
  DAVE_CODEC_AV1 = 6,
} DAVECodec;

typedef enum DAVEEncryptorResultCode {
  DAVE_ENCRYPTOR_RESULT_CODE_SUCCESS = 0,
  DAVE_ENCRYPTOR_RESULT_CODE_ENCRYPTION_FAILURE = 1,
  DAVE_ENCRYPTOR_RESULT_CODE_MISSING_KEY_RATCHET = 2,
  DAVE_ENCRYPTOR_RESULT_CODE_MISSING_CRYPTOR = 3,
  DAVE_ENCRYPTOR_RESULT_CODE_TOO_MANY_ATTEMPTS = 4,
} DAVEEncryptorResultCode;

typedef enum DAVEDecryptorResultCode {
  DAVE_DECRYPTOR_RESULT_CODE_SUCCESS = 0,
  DAVE_DECRYPTOR_RESULT_CODE_DECRYPTION_FAILURE = 1,
  DAVE_DECRYPTOR_RESULT_CODE_MISSING_KEY_RATCHET = 2,
  DAVE_DECRYPTOR_RESULT_CODE_INVALID_NONCE = 3,
  DAVE_DECRYPTOR_RESULT_CODE_MISSING_CRYPTOR = 4,
} DAVEDecryptorResultCode;

typedef void (*DAVEMLSFailureCallback)(const char* source,
                                       const char* reason,
                                       void* user_data);

uint16_t daveMaxSupportedProtocolVersion(void);
void daveFree(void* ptr);

DAVESessionHandle daveSessionCreate(void* context,
                                    const char* auth_session_id,
                                    DAVEMLSFailureCallback callback,
                                    void* user_data);
void daveSessionDestroy(DAVESessionHandle session);
bool daveSessionInit(DAVESessionHandle session,
                     uint16_t version,
                     uint64_t group_id,
                     const char* self_user_id);
void daveSessionReset(DAVESessionHandle session);
uint16_t daveSessionGetProtocolVersion(DAVESessionHandle session);
bool daveSessionSetExternalSender(DAVESessionHandle session,
                                  const uint8_t* external_sender,
                                  size_t length);
bool daveSessionGetMarshalledKeyPackage(DAVESessionHandle session,
                                        uint8_t** key_package,
                                        size_t* length);
bool daveSessionProcessProposals(DAVESessionHandle session,
                                 uint8_t op_type,
                                 const uint8_t* proposals,
                                 size_t length,
                                 const uint64_t* recognized_user_ids,
                                 size_t recognized_user_ids_length,
                                 uint8_t** commit_bytes,
                                 size_t* commit_length,
                                 uint8_t** welcome_bytes,
                                 size_t* welcome_length);
DAVECommitResultHandle daveSessionProcessCommit(DAVESessionHandle session,
                                                const uint8_t* commit,
                                                size_t length);
DAVEWelcomeResultHandle daveSessionProcessWelcome(
    DAVESessionHandle session,
    const uint8_t* welcome,
    size_t length,
    const uint64_t* recognized_user_ids,
    size_t recognized_user_ids_length);
DAVEKeyRatchetHandle daveSessionGetKeyRatchet(DAVESessionHandle session,
                                              const char* user_id);
bool daveSessionGetLastEpochAuthenticator(DAVESessionHandle session,
                                          uint8_t** authenticator,
                                          size_t* length);
bool daveSessionGetEpoch(DAVESessionHandle session, uint64_t* out_epoch);
bool daveSessionGetPairwiseFingerprint(DAVESessionHandle session,
                                       uint16_t version,
                                       const char* user_id,
                                       uint8_t** fingerprint,
                                       size_t* length);

bool daveCommitResultIsFailed(DAVECommitResultHandle result);
bool daveCommitResultIsIgnored(DAVECommitResultHandle result);
void daveCommitResultGetRosterMemberIds(DAVECommitResultHandle result,
                                        uint64_t** roster_ids,
                                        size_t* roster_length);
void daveCommitResultDestroy(DAVECommitResultHandle result);

bool daveWelcomeResultIsFailed(DAVEWelcomeResultHandle result);
void daveWelcomeResultGetRosterMemberIds(DAVEWelcomeResultHandle result,
                                         uint64_t** roster_ids,
                                         size_t* roster_length);
void daveWelcomeResultDestroy(DAVEWelcomeResultHandle result);

void daveKeyRatchetDestroy(DAVEKeyRatchetHandle key_ratchet);
bool daveKeyRatchetDeriveKeyAndNonceForTest(const uint8_t* base_secret,
                                            size_t base_secret_len,
                                            uint32_t generation,
                                            uint8_t* out_key16,
                                            uint8_t* out_nonce12);
bool daveAes128GcmTruncated8EncryptForTest(const uint8_t* key16,
                                           const uint8_t* nonce12,
                                           const uint8_t* aad,
                                           size_t aad_len,
                                           const uint8_t* plaintext,
                                           size_t plaintext_len,
                                           uint8_t* ciphertext_out,
                                           uint8_t* tag8_out);

DAVEEncryptorHandle daveEncryptorCreate(void);
void daveEncryptorDestroy(DAVEEncryptorHandle encryptor);
void daveEncryptorSetKeyRatchet(DAVEEncryptorHandle encryptor,
                                DAVEKeyRatchetHandle key_ratchet);
void daveEncryptorSetPassthroughMode(DAVEEncryptorHandle encryptor,
                                     bool passthrough);
size_t daveEncryptorGetMaxCiphertextByteSize(DAVEEncryptorHandle encryptor,
                                             int32_t media_type,
                                             size_t frame_size);
int32_t daveEncryptorEncrypt(DAVEEncryptorHandle encryptor,
                             int32_t media_type,
                             int32_t codec,
                             const uint8_t* frame,
                             size_t frame_length,
                             uint8_t* encrypted_frame,
                             size_t encrypted_frame_capacity,
                             size_t* bytes_written);

DAVEDecryptorHandle daveDecryptorCreate(void);
void daveDecryptorDestroy(DAVEDecryptorHandle decryptor);
void daveDecryptorTransitionToKeyRatchet(DAVEDecryptorHandle decryptor,
                                         DAVEKeyRatchetHandle key_ratchet);
void daveDecryptorTransitionToPassthroughMode(DAVEDecryptorHandle decryptor,
                                              bool passthrough);
size_t daveDecryptorGetMaxPlaintextByteSize(DAVEDecryptorHandle decryptor,
                                            int32_t media_type,
                                            size_t encrypted_frame_size);
int32_t daveDecryptorDecrypt(DAVEDecryptorHandle decryptor,
                             int32_t media_type,
                             const uint8_t* encrypted_frame,
                             size_t encrypted_frame_length,
                             uint8_t* frame,
                             size_t frame_capacity,
                             size_t* bytes_written);

bool daveTestCreateExternalSender(const uint8_t* label,
                                  size_t label_len,
                                  uint8_t** out_package,
                                  size_t* out_package_len,
                                  void** out_private_handle);
void daveTestDestroyExternalSender(void* private_handle);
bool daveTestSignAddProposal(void* private_handle,
                             uint64_t group_id,
                             uint64_t epoch,
                             const uint8_t* key_package,
                             size_t key_package_len,
                             uint8_t** out_payload,
                             size_t* out_payload_len);
bool daveTestSignRemoveProposal(void* private_handle,
                                uint64_t group_id,
                                uint64_t epoch,
                                uint64_t removed_user_id,
                                uint8_t** out_payload,
                                size_t* out_payload_len);

#ifdef __cplusplus
}
#endif

#endif  // DISCORD_DAVE_C_API_H_
