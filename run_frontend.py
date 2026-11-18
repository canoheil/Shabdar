#!/usr/bin/env python3
import os
import sys
import subprocess
import signal

def main():
    os.chdir(os.path.dirname(os.path.abspath(__file__)))
    web_dir = os.path.join("apps", "web")
    if not os.path.isdir(web_dir):
        print(f"Error: Directory {web_dir} not found.", file=sys.stderr)
        sys.exit(1)

    # Detect the correct npm executable based on the OS
    npm_exec = "npm.cmd" if os.name == "nt" else "npm"

    # Check and install node_modules if missing
    node_modules = os.path.join(web_dir, "node_modules")
    if not os.path.exists(node_modules):
        print("Installing frontend dependencies (npm ci)...")
        res = subprocess.run([npm_exec, "ci"], cwd=web_dir)
        if res.returncode != 0:
            print("Error: npm ci failed.", file=sys.stderr)
            sys.exit(res.returncode)

    env = os.environ.copy()
    env.setdefault("NUXT_API_BASE", "http://127.0.0.1:8080")
    # Nuxt 3 usually uses NUXT_PUBLIC_ for client-side variables.
    # Just in case your nuxt.config.ts expects it:
    env.setdefault("NUXT_PUBLIC_API_BASE", "http://127.0.0.1:8080")

    print("==========================================")
    print("Starting Shabdar Frontend (Nuxt SSR)")
    print("==========================================")
    print(f"Nuxt API Base : {env['NUXT_API_BASE']}")
    print("Frontend URL  : http://localhost:3000")
    print("==========================================")

    process = subprocess.Popen([npm_exec, "run", "dev", "--", "--port", "3000"], cwd=web_dir, env=env)

    def handle_signal(sig, frame):
        print("\nStopping Shabdar Frontend... ")
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
