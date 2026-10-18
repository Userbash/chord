// @ts-check
// Классификация функций по доменам. Контракт: tests/unit/domains.test.mjs
// (паритет extract-domains.sh + domain-report.sh).

/** @typedef {import("../types.js").DomainRow} DomainRow */

/** @type {Array<readonly [string, RegExp]>} */
const RULES = [
  ["webrtc", /[Ww]eb[Rr]tc/],
  ["rtp-rtcp", /Rtp|RTP|rtp_|_rtp|Rtcp|RTCP|rtcp|SRTP|Srtp/],
  ["ice", /(^|[^A-Za-z])ICE([^A-Za-z]|$)|Ice[A-Z]|(^|[^A-Za-z])(ice|ICE)_|icecandidate|icetransport|p2ptransport/],
  ["stun", /[Ss]tun|STUN/],
  ["turn", /[Tt]urn|TURN/],
  ["sdp", /[Ss]dp|SDP|SessionDescription/],
  ["dtls", /[Dd]tls|DTLS/],
  ["rtt", /RTT|Rtt[A-Z]|Rtt$|rtt_|_rtt/],
  ["jitter", /[Jj]itter/],
  ["latency", /[Ll]atency/],
  ["audio", /[Aa]udio|[Oo]pus|OPUS/],
  ["video", /[Vv]ideo|VP8|VP9|H264|H265|AV1|Codec|codec/],
  ["camera", /[Cc]amera|CaptureDevice|VideoCapture|ScreenCast|ScreenShare/],
  ["network", /[Ss]ocket|Network|network|Transport|transport|Connection|connection|Packet|packet|Bandwidth|bandwidth/],
  ["voice", /[Vv]oice|VoiceEngine|MediaConn/],
  ["krisp-noise", /[Kk]risp/],
];

/**
 * Домены демангленного имени: проверяются все правила (как отдельные grep в
 * extract-domains.sh), имя может попасть сразу в несколько доменов.
 * Порядок результата — порядок доменов в таблице:
 * webrtc, rtp-rtcp, ice, stun, turn, sdp, dtls, rtt, jitter, latency,
 * audio, video, camera, network, voice, krisp-noise.
 * @param {string} name демангленное имя, например "RtpTransportControllerSend::OnRttUpdate"
 * @returns {string[]} id доменов, например ["rtp-rtcp", "rtt", "network"]
 */
export function classifyName(name) {
  /** @type {string[]} */
  const matches = [];
  for (const [domain, pattern] of RULES) {
    if (pattern.test(name)) matches.push(domain);
  }
  return matches;
}

/**
 * CSV домена (паритет functions/domains/<domain>.csv):
 * заголовок `module,name,demangled,address,size`; в данных `demangled`
 * в кавычках с удвоением внутренних кавычек, остальные поля — как есть.
 * @param {readonly DomainRow[]} rows
 * @returns {string}
 */
export function domainCsv(rows) {
  return [
    "module,name,demangled,address,size",
    ...rows.map(({ module, name, demangled, address, size }) =>
      `${module},${name},"${demangled.replaceAll('"', '""')}",${address},${size}`,
    ),
  ].join("\n");
}

/**
 * Строка сводки доменов (паритет `printf '%-12s %8d\n'` из extract-domains.sh):
 * id — выровнен влево до 12, пробел, число — вправо до 8.
 * @param {ReadonlyArray<readonly [string, number]>} counts
 * @returns {string} строки через `\n` (без финального `\n`)
 */
export function formatDomainSummary(counts) {
  return counts.map(([domain, count]) => `${domain.padEnd(12)} ${String(count).padStart(8)}`).join("\n");
}
