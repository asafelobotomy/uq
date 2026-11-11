# Integration fixtures

`uq/` folders laid out as an app's repository ships them (Phase 4 spec §2.6), one per extension
point. `tests/shell/integrations_gate.sh` installs each into an empty integration tree, checks
that uQ finds it without a restart, then removes it and checks that it goes. The slices of
Phase 4 add the real apps' folders here, pinned per slice.
