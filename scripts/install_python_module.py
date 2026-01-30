import os
import shutil
import subprocess
import sys
import site
import sysconfig
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
CRATE = ROOT / "pxc-tools"


def run(cmd, cwd=None, env=None):
    subprocess.run(cmd, cwd=cwd, env=env, check=True)


def build_module():
    env = dict(os.environ)
    env["PYO3_USE_ABI3_FORWARD_COMPATIBILITY"] = "1"
    run(["cargo", "build", "--features", "python", "--lib"], cwd=CRATE, env=env)


def find_built_dll():
    for base in (CRATE / "target" / "debug", CRATE / "target" / "release"):
        dll = base / "pxc.dll"
        if dll.exists():
            return dll
    return None


def install():
    build_module()
    dll = find_built_dll()
    if not dll:
        raise RuntimeError("pxc.dll not found; build may have failed")

    site_dir = Path(site.getusersitepackages())
    site_dir.mkdir(parents=True, exist_ok=True)

    # Install native module as pxc.pyd for `import pxc`
    pxc_pyd = site_dir / "pxc.pyd"
    shutil.copy2(dll, pxc_pyd)

    print(f"Installed pxc.pyd to {pxc_pyd}")


if __name__ == "__main__":
    install()
