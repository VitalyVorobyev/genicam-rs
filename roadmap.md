P4 — Stream hardening: MTU/packet size, resend, backpressure, multicast
	•	Auto MTU & packet size: probe NIC MTU, negotiate GevSCPSPacketSize, set GevSCPD (packet delay), detect jumbo frames; expose StreamConfig::auto().
	•	Resend tuning: coalesce missing ranges; bounded retries with jitter; per-frame deadlines; stats (resend reqs, acks, late/duplicate).
	•	Backpressure & zero-copy: ring buffer + preallocated frame pools; Bytes/Arc<[u8]> storage; “drop-oldest” policy when consumer lags; per-stream channel capacity knob.
	•	Multicast receive: configure destination multicast (SFNC), join_multicast() with NIC binding; filter by (src, block_id).
	•	Acceptance: sustain ≥900 Mb/s on 1 GbE with <0.1% drops on clean link; verified resend activity on induced loss; multicast demo across two receivers.

P5 — GenApi engine (Compliance Tier-1)
	•	Node graph: dependency graph, selector propagation, cache invalidation.
	•	Evaluators: Integer/Float with increment, min/max; Enum (entries/values); Boolean; Command with OnUpdate; visibility/access mode; SwissKnife formulas (subset).
	•	Reg map: address/length mapping, concatenated registers, endian helpers.
	•	SFNC helpers: typed getters/setters for common features; feature watchers.
	•	Acceptance: read/write Exposure/Gain/TriggerMode/PixelFormat end-to-end via node map (no raw register calls), selectors honored.

P6 — Tooling & UX
	•	CLI gencamctl: list, features get/set, start stream → PGM/PPM/BMP dump, events view, chunk dump, multicast join; --iface, --mtu, --packet-size, --delay.
	•	Bench gencam-bench: throughput/latency/resend/drops; JSON report.
	•	Windows support: socket opts, RCVBUF, NIC binding; firewall notes.
	•	Acceptance: CI builds on Linux/macOS/Windows; soak test (10 min) stable.

⸻
