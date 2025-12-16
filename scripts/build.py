import argparse
import os
import platform
import subprocess

from base import green


def run_cmd(cmd: list[str]) -> None:
    print(f"{green('Running')}: {' '.join(cmd)}", flush=True)
    subprocess.run(cmd, text=True, check=True)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("cmd", choices=["check", "test", "build", "clippy"])
    parser.add_argument("--release", action="store_true")
    parser.add_argument("-e", "--examples", type=str, nargs="*")
    opts = parser.parse_args()

    cmd = ["cargo", opts.cmd]

    if opts.examples:
        for e in opts.examples:
            os.chdir("examples/" + e)
            cmd.append("--release")
            run_cmd(cmd)
            os.chdir("../../")
    elif opts.cmd == "test":
        cmd.append("--features=std")
        if platform.system().lower() == "windows":
            cmd.append("--target=x86_64-pc-windows-msvc")
        else:
            cmd.append("--target=x86_64-unknown-linux-gnu")
        run_cmd(cmd)
    else:
        if opts.release:
            cmd.append("--release")

        cmd.append(f"--features=stm32f103,xG")
        run_cmd(cmd)

    return 0


if __name__ == "__main__":
    ret = 0

    try:
        ret = main()
    except KeyboardInterrupt as e:
        print(e)
        ret = -1
    except subprocess.CalledProcessError as e:
        print(e)
        ret = e.returncode

    exit(ret)
