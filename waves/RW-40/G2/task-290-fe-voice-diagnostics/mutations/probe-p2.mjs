import {
  createVoiceDiagnosticsController,
  evaluatePingQuality,
  formatPopoutMetricsRu,
  RTC_DEBUG_SECTIONS,
} from '/home/sanya/orca/workspaces/project/feature-task-290-fe-voice-diagnostics/rust/gui-spike/ui/features/voice-diagnostics.js';

const c = createVoiceDiagnosticsController();
c.updateSnapshot({ privacyCodeChunks: ['abc', 'de'] });
console.log('chunks ["abc","de"] copy ->', c.copyPrivacyCode(), c.getState().copiedPrivacyCode);

const c2 = createVoiceDiagnosticsController();
c2.updateSnapshot({ privacyCodeChunks: ['12345', '67890'] });
let got = null;
const c3 = createVoiceDiagnosticsController({ onCopyPrivacyCode: (code) => { got = code; } });
c3.updateSnapshot({ privacyCodeChunks: ['12345', '67890'] });
console.log('short chunks copy ->', c3.copyPrivacyCode(), 'payload=', got);

console.log('loss 101 ->', evaluatePingQuality(50, 101), '(оракул: bad)');
console.log('loss 100.0001 ->', evaluatePingQuality(50, 100.0001));
console.log('ping -0 ->', evaluatePingQuality(-0));
console.log('infinity avg ping label ->', formatPopoutMetricsRu({ averagePingMs: Infinity }).averagePingLabel);
console.log('negative ping label ->', formatPopoutMetricsRu({ lastPingMs: -42 }).lastPingLabel);
console.log('sections ->', JSON.stringify([...RTC_DEBUG_SECTIONS]));

const c4 = createVoiceDiagnosticsController();
const pings = [10, 20];
c4.updateSnapshot({ pings });
pings.push(9999);
console.log('nested snapshot pings shared ->', JSON.stringify(c4.getState().snapshot?.pings));
