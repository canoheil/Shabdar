#!/usr/bin/env python3
import os
import sys
import subprocess
import signal

def main():
    os.chdir(os.path.dirname(os.path.abspath(__file__)))
    api_dir = os.path.join("apps", "api")
    if not os.path.isdir(api_dir):
        print(f"Error: Directory {api_dir} not found.", file=sys.stderr)
        sys.exit(1)

    # Set default environment variables if not provided
    env = os.environ.copy()
    env.setdefault("DATABASE_URL", "mysql://shabdar:shabdar123@127.0.0.1:3306/shabdar_dev")
    env.setdefault("BIND_ADDR", "127.0.0.1:8080")
    if not env.get("OTP_PEPPER"):
        import secrets
        env["OTP_PEPPER"] = secrets.token_hex(32)

    print("==========================================")
    print("Starting Shabdar Backend (Rust / Axum)")
    print("==========================================")
    print(f"Database URL : {env['DATABASE_URL']}")
    print(f"Bind Address : {env['BIND_ADDR']}")
    print("==========================================")

    process = subprocess.Popen(["cargo", "run"], cwd=api_dir, env=env)

    def handle_signal(sig, frame):
        print("\nStopping Shabdar Backend...")
        process.terminate()
        try:
            process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            process.kill()
        sys.exit(0)

    signal.signal(signal.SIGINT, handle_signal)
    if hasattr(signal, "SIGTERM"):
        signal.signal(signal.SIGTERM, handle_signal)

    return_code = process.wait()
    sys.exit(return_code)

if __name__ == "__main__":
    main()
