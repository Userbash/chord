# G1 Auth & Network — inbox

[2026-10-24] [G1] [A2] accepted
WHAT: captcha-поля/headers, 95/0, clippy/fmt 0.
EVIDENCE: D-40, cargo test gui-spike.

[2026-10-24] [G1] [A3b] accepted
WHAT: SupervisorEvent callback + AuthBackend::token; core 573/0.
EVIDENCE: D-42.

[2026-10-24] [G1] [A3c] partial
WHAT: NetController/FakeGateway, 101/0; production driver + Tauri commands остались.
NEXT: A3d — CoreGatewayDriver на Supervisor, net_* commands.
