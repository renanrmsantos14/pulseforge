# PulseForge project instructions

After every source change that affects the application, finalize the change
with `./scripts/release-local.sh` (bumps the version and builds), commit, then
publish with `./scripts/publish-release.sh`.

Updates: the app checks the latest GitHub Release of `renanrmsantos14/pulseforge`
(asset `pulseforge-linux-x86_64`) via `curl`. No telemetry and no other network
dependency.
