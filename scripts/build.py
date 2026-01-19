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
    parser.add_argument("--features", type=str, nargs="*")
    parser.add_argument("-e", "--example", type=str, default="")
    opts = parser.parse_args()

    cmd = ["cargo", opts.cmd]

    if opts.example:
        if opts.features:
            for ft in opts.features:
                cmd.append(f"--features={ft}")
        os.chdir("examples/" + opts.example)
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
        cmd.append("--features=f103,xG")
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
