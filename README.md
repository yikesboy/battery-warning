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
| `--low-theme DIR`        | Low warning theme directory. Required.                         |
| `--critical-theme DIR`   | Critical warning theme directory. Required.                    |
| `--low-threshold N`      | Warn at or below this percentage. Default `15`.                |
| `--critical-threshold N` | Critical warning at or below this percentage. Default `5`.     |
| `--rearm-threshold N`    | Warnings reset once the battery goes above this. Default `20`. |

Thresholds have to be `0 <= critical < low < rearm <= 100`.

Each dialog runs with its theme directory as `XDG_CONFIG_HOME`.
GTK reads `gtk-4.0/gtk.css` from that directory.

## NixOS and Home Manager

1. Add the flake input. Keep its own nixpkgs pin (no `follows`).

```nix
inputs.battery-warning.url = "github:yikesboy/battery-warning";
```

2. Enable UPower and pass `inputs` to Home Manager in your NixOS configuration:

```nix
services.upower.enable = true;
home-manager.extraSpecialArgs = { inherit inputs; };
```

3. Add this Home Manager module:

```nix
{ inputs, pkgs, lib, ... }:
let
  batteryWarning = inputs.battery-warning.packages.${pkgs.stdenv.hostPlatform.system}.default;
  theme = pkgs.writeTextDir "gtk-4.0/gtk.css" "";
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
        "--low-theme" "${theme}"
        "--critical-theme" "${theme}"
      ];
      Restart = "on-failure";
      RestartSec = "5s";
    };
  };
}
```

4. Ensure your desktop manages `graphical-session.target` and supplies its display
   environment to the systemd user manager.

5. Rebuild your NixOS configuration and log out and back in. The service starts
   and stops with your graphical session.
