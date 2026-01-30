import os
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
CRATE = ROOT / "pxc-tools"


def run(cmd, cwd=None, env=None):
    subprocess.run(cmd, cwd=cwd, env=env, check=True)


def main():
    env = dict(os.environ)
    env["PYO3_USE_ABI3_FORWARD_COMPATIBILITY"] = "1"
    run(["cargo", "build", "--features", "python", "--lib"], cwd=CRATE, env=env)


if __name__ == "__main__":
    main()
