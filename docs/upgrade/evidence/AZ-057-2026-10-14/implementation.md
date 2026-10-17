# AZ-057 Windows resource sampler candidate (2026-10-14)

Added `scripts/upgrade-resource-sampler.ps1`, a bounded Windows process-tree sampler with an explicit root PID and JSONL output. It pins the root creation time to reject PID reuse, traces current descendants, and records discovered/measured process counts, working set, handles, threads and cumulative CPU. It writes no command lines or environment variables. Sampling ends when the root exits or the requested duration is reached; a slow CIM census can make wall time exceed the nominal duration.

A two-sample self-check against the invoking PowerShell process passed and is saved in [sampler-self-check.jsonl](sampler-self-check.jsonl). This demonstrates script execution only. It is not an AgentZ long-running benchmark, fault-injection test, two-hour soak, native cold-start result or real-model run.

The release benchmark still needs a frozen build/hardware profile, reviewed thresholds and a repeatable AgentZ workload after AZ-026/AZ-032. AZ-057 remains blocked.

## Follow-up, 2026-10-14

The sampler now retains process identities seen during earlier censuses. Previously it stopped as soon as the root exited, concealing surviving descendants and making a leak appear as zero. It now continues measuring known descendants after root exit until they exit or the duration expires, and checks each PID's creation time before reuse. A 2-second local self-check passed. A separate hidden PowerShell fixture spawned a 15-second child and exited after 4 seconds; the sampler recorded subsequent samples with `root_alive=false` and two still-measured descendants. This is a sampler fixture, not an AgentZ soak or fault benchmark.

The regression is now reproducible with `scripts/upgrade-resource-sampler.verify.ps1`: it starts a synthetic parent and child, checks that the parent exits while a descendant remains measured, and removes its temporary files. The verifier passed with six samples. It does not claim AgentZ resource budgets or long-duration stability.
