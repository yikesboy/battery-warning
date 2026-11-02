# Battery Warning

A Linux user-session listener that watches UPower's aggregate `DisplayDevice`
on the system D-Bus and displays acknowledgeable Zenity warnings. It takes an
initial snapshot, then reacts to property changes rather than polling. It does
not suspend, shut down, or otherwise manage power.

## Build and Check

```sh
nix develop -c cargo test --locked
nix develop -c cargo fmt --check
nix develop -c cargo clippy --offline --locked --all-targets -- -D warnings
nix build
nix flake check
```

The development shell includes the pinned Rust toolchain and D-Bus for the
integration tests. The first development Cargo command may need network access
to populate Cargo's cache before offline Clippy can run.

`nix build` produces `result/bin/battery-warning` and runs the tests, including
private D-Bus and shell-based fake-dialog tests. No real UPower service, display,
or Zenity is needed for tests. `nix flake check` reuses that package and also
checks formatting, all-target Clippy with warnings denied, and the toolchain.
Nix vendors dependencies from `Cargo.lock`; Cargo tests and Clippy run offline
and locked inside the derivations. The lint derivation skips the redundant
release build. The `rust-toolchain` package remains available separately.

Nix's Git source filtering excludes untracked files. New source and test files
must be included in Git before the final checks.

## CLI

```sh
./result/bin/battery-warning \
  --zenity /absolute/path/to/zenity \
  --low-theme /absolute/path/to/low-theme \
  --critical-theme /absolute/path/to/critical-theme \
  --low-threshold 15 \
  --critical-threshold 5 \
  --rearm-threshold 20
```

| Option | Meaning |
| --- | --- |
| `--zenity PATH` | Required absolute path to a regular executable Zenity file. Use GTK 4 Zenity. |
| `--low-theme DIR` | Required theme directory containing readable `gtk-4.0/gtk.css`. |
| `--critical-theme DIR` | Required theme directory containing readable `gtk-4.0/gtk.css`. |
| `--low-threshold NUMBER` | Low warning at or below this percentage; default `15`. |
| `--critical-threshold NUMBER` | Critical warning at or below this percentage; default `5`. |
| `--rearm-threshold NUMBER` | Rearm only strictly above this percentage; default `20`. |
| `--help`, `--version` | Print CLI help or version and exit. |

Thresholds must be finite and satisfy
`0 <= critical < low < rearm <= 100`. Theme paths may be relative, but absolute
paths are recommended for services. Each child receives its theme directory as
`XDG_CONFIG_HOME`, so GTK reads `DIR/gtk-4.0/gtk.css`; this does not change the
listener's environment or install a global theme. Supply your own CSS for each
severity. No personal palette or theme is bundled.

Warnings are issued only when UPower reports on-battery power, a present display
battery, and a discharging or pending-discharge state. A startup snapshot already
below a threshold warns immediately. At most one dialog is owned at a time;
critical replaces and reaps an open low dialog. The titles are exactly
`Battery Low` and `Battery Critical`.

Clicking OK or closing a dialog (Zenity exit codes `0` or `1`) acknowledges it.
The same severity does not repeat in that discharge cycle, but low can escalate
to critical. AC power, an absent battery, or a percentage strictly above the
rearm threshold closes the dialog and resets warning state. Exactly the rearm
threshold does not reset it. Warning state is in memory and resets on restart.

## Runtime and Failure

Run in a graphical user session with access to the system D-Bus, a running
UPower service, and a working GTK 4 Zenity executable. Zenity inherits the
session's display environment. Runtime dependencies and themes are supplied by
the consumer, not installed or enabled by this flake.

Invalid configuration, missing or unreadable UPower properties, invalid battery
percentages, bus/stream errors, UPower disappearance or bus-owner replacement,
and unexpected Zenity failures cause a nonzero exit. The listener does not
silently reconnect or retry internally. A supervisor can restart it after
failure; a restart takes a fresh snapshot and may warn again. SIGTERM/SIGINT
shut down the listener, terminating and reaping its owned dialog. Diagnostics go
to stderr (the journal under systemd).

## Consumer Example

This is an example for a separate consumer flake and Home Manager configuration,
not a deployment performed by this repository. Replace the example repository
URL and supply your own theme directories. Keep this input's own nixpkgs pin;
do not add an `inputs.nixpkgs.follows` override to it.

```nix
# In the consumer flake's inputs:
inputs.battery-warning.url = "github:YOUR-ORG/battery-notification-util";
```

Pass the consumer's `inputs` to the Home Manager module, for example through
`home-manager.extraSpecialArgs = { inherit inputs; };`, then use:

```nix
{ inputs, pkgs, lib, ... }:
let
  batteryWarning = inputs.battery-warning.packages.${pkgs.stdenv.hostPlatform.system}.default;
  lowTheme = ./themes/battery-low;
  criticalTheme = ./themes/battery-critical;
in {
  systemd.user.services.battery-warning = {
    Unit = {
      Description = "Battery warning dialogs";
      After = [ "graphical-session.target" ];
      PartOf = [ "graphical-session.target" ];
    };
    Install.WantedBy = [ "graphical-session.target" ];
    Service = {
      ExecStart = lib.escapeShellArgs [
        "${batteryWarning}/bin/battery-warning"
        "--zenity" "${pkgs.zenity}/bin/zenity"
        "--low-theme" "${lowTheme}"
        "--critical-theme" "${criticalTheme}"
        "--low-threshold" "15"
        "--critical-threshold" "5"
        "--rearm-threshold" "20"
      ];
      Restart = "on-failure";
      RestartSec = "5s";
    };
  };
}
```

Each referenced theme directory must contain `gtk-4.0/gtk.css`. Enable UPower
in the consumer's system configuration. Ensure the compositor/session manages
`graphical-session.target` and imports its display environment into the systemd
user manager before starting the service. Do not attach this to a boot-wide
system service or a lingering user's `default.target` without a graphical
session. For Home Manager Sway, session integration can be provided by
`wayland.windowManager.sway.systemd.enable = true;`.

Optional Sway rules, using the exact titles, can make the dialogs float without
imposing any colors or other personal styling:

```sway
for_window [title="^Battery Low$"] floating enable
for_window [title="^Battery Critical$"] floating enable
```

No external configuration is modified and no service is deployed by building
or checking this project.
