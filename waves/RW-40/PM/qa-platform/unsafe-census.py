import re
files=["rust/crates/discord-store/src/ffi.rs","rust/crates/discord-platform/src/ffi_secret.rs","rust/crates/discord-platform/src/ffi_hunspell.rs","rust/crates/discord-platform/src/dbus/fd.rs"]
import os
os.chdir("/home/user/orca/project")
for f in files:
    lines=open(f,encoding="utf-8").read().split("\n")
    print("##",f)
    n=0
    for i,l in enumerate(lines):
        if "unsafe {" in l:
            n+=1
            # enclosing fn
            fn="?"
            for j in range(i,-1,-1):
                m=re.search(r'\bfn\s+([a-z_0-9]+)',lines[j])
                if m and not lines[j].strip().startswith("//"):
                    fn=m.group(1); break
            # nearest SAFETY above
            dist=None
            for j in range(i-1,max(-1,i-8),-1):
                if "SAFETY" in lines[j]:
                    dist=i-j; break
            print(f"{i+1}\t{fn}\tSAFETY_above={dist}")
    print("count",n)
