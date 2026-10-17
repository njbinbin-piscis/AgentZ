// Synthetic kernel baseline launcher. No SDK/network calls; paid test is explicit --run.
import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import { spawn } from 'node:child_process';
import { fileURLToPath } from 'node:url';
const root=path.resolve(path.dirname(fileURLToPath(import.meta.url)),'..');
const args=process.argv.slice(2);
const run=args.includes('--run');
function option(name,fallback){const i=args.indexOf(name);return i<0?fallback:args[i+1];}
const output=path.resolve(option('--output',path.join(root,'target/model-baseline-v1.json')));
const repeats=Number(option('--repeats','1'));
if(!Number.isInteger(repeats)||repeats<1||repeats>3)throw new Error('Repeats must be 1..3');
const source=fs.readFileSync(path.join(root,'vendor/piscis-engine/piscis-kernel/tests/upgrade_live_baseline.rs'),'utf8');
const raw=source.match(/const SPEC_JSON: &str = r#"([\s\S]*?)"#;/)?.[1];
if(!raw)throw new Error('Fixed synthetic baseline spec missing');
const spec=JSON.parse(raw);
if(spec.length!==12||new Set(spec.map(v=>v.id)).size!==12)throw new Error('Expected fixed 12 distinct tasks');
const specHash=crypto.createHash('sha256').update(raw).digest('hex');
const sourceHash=crypto.createHash('sha256').update(source).digest('hex');
fs.mkdirSync(path.dirname(output),{recursive:true});
const manifest={schema_version:1,baseline:'first-fixed-synthetic-v1',spec_sha256:specHash,fixture_source_sha256:sourceHash,repeats,model:'deepseek-flash',endpoint:'https://api.deepseek.com',thinking:false,maximum_samples:12*repeats,maximum_logical_calls:48*repeats,planned_maximum_output_tokens:12288*repeats,maximum_request_payload_bytes:196608*repeats,budget_note:'request-byte proxy and logical-call/output caps; provider internal wire retries/usage/prices not a hard billing guarantee',scope:'real kernel synthetic fixture tools; not host/GUI/OS or priced settlement',tasks:spec};
fs.writeFileSync(`${output}.spec.json`,JSON.stringify(manifest,null,2)+'\n');
if(!run){console.log(JSON.stringify({status:'frozen_not_run',spec_sha256:specHash,samples:12*repeats,manifest:`${output}.spec.json`}));}
else {
 const secret=process.env.AGENTZ_BASELINE_API_KEY;
 if(!secret?.trim())throw new Error('Temporary AGENTZ_BASELINE_API_KEY environment required');
 // Never pass credentials as command arguments/config files. Child output is not printed.
 const child=spawn('cargo',['test','-p','piscis-kernel','--test','upgrade_live_baseline','--offline','--locked','--','--ignored','--nocapture'],{cwd:root,windowsHide:true,env:{...process.env,AGENTZ_BASELINE_SPEC_SHA256:specHash,AGENTZ_BASELINE_SOURCE_SHA256:sourceHash,AGENTZ_BASELINE_REPEATS:String(repeats),AGENTZ_BASELINE_OUTPUT:output},stdio:['ignore','pipe','pipe']});
 let bytes=0;let limitHit=false;
 for(const pipe of [child.stdout,child.stderr])pipe.on('data',chunk=>{bytes+=chunk.length;if(bytes>16*1024*1024){limitHit=true;child.kill();}});
 const deadline=setTimeout(()=>child.kill(),30*60*1000);
 child.on('error',()=>{clearTimeout(deadline);console.error('Baseline child could not start');process.exitCode=1;});
 child.on('close',code=>{clearTimeout(deadline);console.log(JSON.stringify({status:code===0&&!limitHit?'execution_finished':'execution_failed',exit_code:code,output_exists:fs.existsSync(output),spec_sha256:specHash,log_output_suppressed:true}));process.exitCode=code===0&&!limitHit?0:1;});
}
