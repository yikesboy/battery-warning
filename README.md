# Battery Warning

An event-driven Linux user-session listener that watches UPower over the system
D-Bus and pops up Zenity warnings when the battery gets low.

## Build and Check

```sh
nix develop -c cargo test --locked
nix develop -c cargo fmt --check
nix develop -c cargo clippy --offline --locked --all-targets -- -D warnings
nix build
nix flake check
```

## Usage

```sh
./result/bin/battery-warning \
  --zenity /absolute/path/to/zenity \
  --low-theme /absolute/path/to/low-theme \
  --critical-theme /absolute/path/to/critical-theme \
  --low-threshold 15 \
  --critical-threshold 5 \
  --rearm-threshold 20
```

| Option                   | Meaning                                                        |
| ------------------------ | -------------------------------------------------------------- |
| `--zenity PATH`          | Absolute path to a GTK 4 Zenity executable. Required.          |
| `--low-theme DIR`        | Theme directory containing `gtk-4.0/gtk.css`. Required.        |
| `--critical-theme DIR`   | Theme directory for the critical dialog. Required.             |
| `--low-threshold N`      | Warn at or below this percentage. Default `15`.                |
| `--critical-threshold N` | Critical warning at or below this percentage. Default `5`.     |
| `--rearm-threshold N`    | Warnings reset once the battery goes above this. Default `20`. |

Thresholds have to be `0 <= critical < low < rearm <= 100`.

Each dialog runs with its theme directory as `XDG_CONFIG_HOME`.
GTK loads that severity's `gtk-4.0/gtk.css`.

## Home Manager example

Add the input to your flake without a `follows` for nixpkgs.

```nix
inputs.battery-warning.url = "github:YOUR-ORG/battery-notification-util";
```

Then add this module, with `inputs` passed through
`home-manager.extraSpecialArgs`.

```nix
{ inputs, pkgs, lib, ... }:
let
  batteryWarning = inputs.battery-warning.packages.${pkgs.stdenv.hostPlatform.system}.default;
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
        "--low-theme" "${./themes/battery-low}"
        "--critical-theme" "${./themes/battery-critical}"
      ];
      Restart = "on-failure";
      RestartSec = "5s";
    };
  };
}
```

You'll also need UPower enabled on the system, and your compositor must start
`graphical-session.target` and pass its display environment to the systemd user
manager.
