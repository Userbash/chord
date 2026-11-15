import os, subprocess, sys
subprocess.run(["/usr/sbin/ip", "link", "set", "lo", "up"], check=True)
os.execv(sys.argv[1], sys.argv[1:])
