from pathlib import Path
import datetime,json,os,subprocess,sys,hashlib
p=Path('/root/projects/chord-modernization');out=Path(__file__).parent
phase=sys.argv[1];command=sys.argv[2:];stamp=datetime.datetime.now(datetime.timezone.utc).strftime('%Y%m%dT%H%M%S%fZ')
log=out/(phase+'-'+stamp+'.log');env=dict(os.environ);env['PATH']='/root/.cargo/bin:/root/.local/share/mcp/node-v24.22.3-linux-x64/bin:'+env['PATH'];env['CARGO_BUILD_JOBS']='4';env['PULSE_SERVER']='unix:/tmp/chord-coalescing-no-audio-server'
print(json.dumps({'command':command,'log':str(log)}),flush=True)
with log.open('x') as stream:r=subprocess.run(command,cwd=p,env=env,stdout=stream,stderr=subprocess.STDOUT)
receipt={'command':command,'exit_code':r.returncode,'log':str(log),'log_sha256':hashlib.sha256(log.read_bytes()).hexdigest(),'source_commit':subprocess.check_output(['git','rev-parse','HEAD'],cwd=p,text=True).strip(),'source_sha256':hashlib.sha256((p/'rust/gui-spike/src/data.rs').read_bytes()).hexdigest(),'test_sha256':hashlib.sha256((p/'rust/gui-spike/tests/server_join_and_subscriptions_contract.rs').read_bytes()).hexdigest()}
log.with_suffix('.json').write_text(json.dumps(receipt,indent=2)+'\n');print(json.dumps(receipt),flush=True);print('\n'.join(log.read_text(errors='replace').splitlines()[-30:]),flush=True);sys.exit(r.returncode)
