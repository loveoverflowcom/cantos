// xtask-only, loopback fixture; no dependencies, account credentials, issuer or external services.
// CDP drives installed Chromium input events against the built production Web modules.
import http from 'node:http';
import {spawn,execFileSync} from 'node:child_process';
import {readFile,writeFile,mkdir,mkdtemp,rm,access} from 'node:fs/promises';
import {resolve,join,extname} from 'node:path';
import {tmpdir} from 'node:os';
import {readFileSync} from 'node:fs';
import {createHash,randomBytes,randomUUID} from 'node:crypto';
import assert from 'node:assert/strict';
const root=resolve(import.meta.dirname,'..');
const chrome=process.env.CANTOS_TEST_CHROMIUM || '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome';
await access(chrome);
if(typeof WebSocket!=='function') throw new Error('installed Node must provide WebSocket (Node22+)');
if(process.argv.includes('--probe')){console.log('PASS installed Chromium and Node WebSocket; no install or launch');process.exit(0);}
const run=join(root,'artifacts/agent-runs',`web-bearer-${Date.now()}`);await mkdir(run,{recursive:true,mode:0o700});
const source=()=>{
 const hash=createHash('sha256');const files=execFileSync('git',['ls-files','--cached','--others','--exclude-standard','-z'],{cwd:root}).toString().split('\0').filter(Boolean).sort();
 for(const file of files){hash.update(file+'\0');try{hash.update(readFileSync(join(root,file)));}catch{hash.update('deleted');}}
 return {head:execFileSync('git',['rev-parse','HEAD'],{cwd:root,encoding:'utf8'}).trim(),content_fingerprint:hash.digest('hex')};
};
const before=source();
const origins={studio:'http://localhost:18081',theatre:'http://localhost:18082'};
const apiOrigin='http://127.0.0.1:18080',issuerOrigin='http://127.0.0.1:18083';
const state={flows:new Map(),families:new Map(),tokens:new Map(),refreshes:0,revokes:0,cancels:0,exchanges:0,requests:[],imports:new Map(),images:new Map(),artwork:null,imagePuts:0,cookieProbes:[],holdRefresh:null,dropRefresh:false,holdLibrary:null,failRevoke:false,shortAccess:false,denied:false};
const secret=()=>randomBytes(32).toString('hex');
const workId='00000000-0000-4000-8000-000000000077';
const actorA='00000000-0000-4000-8000-000000000001',actorB='00000000-0000-4000-8000-000000000099';
const json=(res,status,body,extra={})=>{res.writeHead(status,{'content-type':'application/json','cache-control':'no-store',...extra});res.end(JSON.stringify(body));};
const error=(res,status,code='unavailable')=>json(res,status,{code,current_revision:null,issues:[]});
function pair(surface,actor,family){
 const time=Math.floor(Date.now()/1000);const session=family||{id:randomUUID(),actor,surface,absolute:time+600,auth:time,revoked:false,spent:new Set()};
 const accessToken=`ca1_${surface==='studio'?'s':'t'}_${secret()}`,refreshToken=`cr1_${secret()}`;
 const result={token_type:'Bearer',access_token:accessToken,refresh_token:refreshToken,actor_id:actor,session_id:session.id,expires_at:time+(state.shortAccess?1:120),refresh_expires_at:Math.min(time+300,session.absolute),absolute_expires_at:session.absolute,authenticated_at:session.auth};
 session.access=accessToken;session.refresh=refreshToken;session.view=result;state.families.set(session.id,session);state.tokens.set(accessToken,session);return result;
}
async function body(req){let value='';for await(const b of req){value+=b;if(value.length>2**20)throw new Error('fixture body limit');}return value?JSON.parse(value):{};}
const api=http.createServer(async(req,res)=>{try{
 const url=new URL(req.url,apiOrigin),surface=url.pathname.startsWith('/api/v1/studio/')?'studio':'theatre';
 const origin=req.headers.origin;
 if(origin&&origin!==origins[surface])return error(res,403,'forbidden');
 if(origin){res.setHeader('access-control-allow-origin',origin);res.setHeader('vary','Origin');res.setHeader('access-control-expose-headers','x-cantos-studio-session,x-cantos-theatre-session,cantos-provider-revocation');}
 if(req.method==='OPTIONS'){res.writeHead(204,{'access-control-allow-methods':'GET,POST,PUT,PATCH,DELETE,OPTIONS','access-control-allow-headers':'authorization,content-type,x-cantos-studio-session,x-cantos-theatre-session'});return res.end();}
 if(url.pathname==='/_test/cookie-control'){state.cookieProbes.push({origin:origin||null,cookie:!!req.headers.cookie});json(res,200,{cookie:!!req.headers.cookie},{'access-control-allow-credentials':'true'});return;}
 if(url.pathname==='/favicon.ico'){res.writeHead(204);return res.end();}
 if(req.headers.cookie){state.requests.push({surface,path:url.pathname,cookie:true});return error(res,400,'invalid_request');}
 state.requests.push({surface,path:url.pathname,method:req.method,authorization:!!req.headers.authorization,cookie:false});
 const prefix=`/api/v1/${surface}/auth/bearer`,suffix=url.pathname.slice(prefix.length);
 if(url.pathname===`/api/v1/${surface}/auth/configuration`)return json(res,200,surface==='studio'?{mode:'oidc'}:{login:'oidc',registration:{status:'unavailable',reason:'registration_unconfigured'}});
 if(surface==='theatre'&&url.pathname===`/api/v1/${surface}/works`)return json(res,200,{items:[],next_cursor:null});
 if(url.pathname.startsWith(prefix)&&req.method==='POST'){
  assert(!req.headers.authorization,'protocol request mixed credentials');const b=await body(req);
  if(suffix==='/flows'){
   assert(/^[\w.~\-]{43,128}$/.test(b.state));assert(/^[\w.~\-]{43,128}$/.test(b.nonce));assert(/^[\w\-]{43}$/.test(b.code_challenge));
   const id=randomUUID();state.flows.set(id,{id,surface,state:b.state,challenge:b.code_challenge,reauthenticate:b.reauthenticate,code:secret(),cancelled:false,actor:actorA});
   return json(res,201,{flow_id:id,authorization_url:`${issuerOrigin}/authorize?flow=${id}`});
  }
  const match=suffix.match(/^\/flows\/([\w-]+)\/(exchange|cancel)$/);
  if(match){const flow=state.flows.get(match[1]);assert(flow&&flow.surface===surface&&flow.state===b.state);assert.equal(createHash('sha256').update(b.code_verifier).digest('base64url'),flow.challenge);
   if(match[2]==='cancel'){state.cancels++;flow.cancelled=true;if(flow.family)state.families.get(flow.family).revoked=true;res.writeHead(204);return res.end();}
   state.exchanges++;assert(!flow.exchanged&&!flow.cancelled&&flow.code===b.code);flow.exchanged=true;const tokens=pair(surface,flow.actor);flow.family=tokens.session_id;return json(res,200,tokens);
  }
  const family=state.families.get(b.session_id);assert(family&&family.surface===surface);assert(b.refresh_token===family.refresh||family.spent.has(b.refresh_token));
  if(suffix==='/revoke'){state.revokes++;if(state.failRevoke)return error(res,503);family.revoked=true;res.writeHead(204,{'cantos-provider-revocation':'pending'});return res.end();}
  if(suffix==='/refresh'){
   state.refreshes++;
   if(family.revoked||family.spent.has(b.refresh_token)){family.revoked=true;return error(res,401,'unauthenticated');}
   family.spent.add(family.refresh);const rotated=pair(surface,family.actor,family);
   if(state.holdRefresh)await state.holdRefresh;
   if(state.dropRefresh){state.dropRefresh=false;res.writeHead(200,{'content-type':'application/json','cache-control':'no-store','content-length':'128','connection':'close'});res.end('{');return;}
   return json(res,200,rotated);
  }
 }
 const token=req.headers.authorization?.replace(/^Bearer /,'');const family=state.tokens.get(token);
 if(!family||family.revoked||family.access!==token||family.surface!==surface||state.denied)return error(res,401,'unauthenticated');
 res.setHeader(`x-cantos-${surface}-session`,family.id);
 if(url.pathname==='/api/v1/studio/works')return json(res,200,{items:[{id:workId,kind:'novel',owner_id:actorA,details:{title:'Bìa truyện thử nghiệm riêng tư',synopsis:'Fixture only',language:'vi-VN',visibility:'private',rights_refs:['synthetic_fixture']},lifecycle:'draft',revision:1,created_at:1,updated_at:1}],next_cursor:null});
 const workBase=`/api/v1/studio/works/${workId}`;
 if(url.pathname===`${workBase}/content-units`)return json(res,200,{items:[],next_cursor:null});
 if(url.pathname===`${workBase}/delegations`)return json(res,200,{items:[],next_after:null});
 if(url.pathname===`${workBase}/artwork`){
  if(req.method==='PATCH'){const b=await body(req);state.artwork=b.artwork;return json(res,200,{work_id:workId,work_revision:b.expected_work_revision+1,artwork:b.artwork,changed_at_ms:Date.now()});}
  return json(res,200,{work_id:workId,work_revision:1,artwork:state.artwork,changed_at_ms:null});
 }
 if(url.pathname===`${workBase}/image-intents`&&req.method==='POST'){
  const b=await body(req),intent={schema_version:'1.0',id:b.id,work_id:workId,owner_id:family.actor,operation_id:b.operation_id,request:b,created_at_ms:Date.now(),expires_at_ms:Date.now()+300000,digest:secret(),profile_id:'mock-private-png-v1'};
  state.images.set(intent.id,{intent,asset:null,bytes:null});return json(res,201,intent);
 }
 const imageIntent=url.pathname.match(new RegExp(`^${workBase}/image-intents/([a-f0-9-]+)(/bytes)?$`));
 if(imageIntent){const image=state.images.get(imageIntent[1]);assert(image);
  if(imageIntent[2]){state.imagePuts++;const parts=[];for await(const chunk of req)parts.push(chunk);image.bytes=Buffer.concat(parts);assert.equal(image.bytes.length,image.intent.request.expected_byte_length);assert.equal(createHash('sha256').update(image.bytes).digest('hex'),image.intent.request.expected_sha256);
   image.asset={schema_version:'1.0',id:randomUUID(),intent_id:image.intent.id,work_id:workId,owner_id:family.actor,verified_media:{sha256:image.intent.request.expected_sha256,byte_length:image.bytes.length,content_type:'image/png',width_px:1,height_px:1,profile_id:'mock-private-png-v1'},declarations:image.intent.request.declarations,admitted_at_ms:Date.now(),digest:secret()};
   // Commit then truncate the response body: production XHR must reconcile its immutable receipt.
   res.writeHead(200,{'content-type':'application/json','content-length':'128','connection':'close'});return res.end('{');
  }
  return json(res,200,{intent:image.intent,asset:image.asset,state:image.asset?'admitted':'pending'});
 }
 const imageAsset=url.pathname.match(new RegExp(`^${workBase}/image-assets/([a-f0-9-]+)(/bytes)?$`));
 if(imageAsset){const image=[...state.images.values()].find(image=>image.asset?.id===imageAsset[1]);assert(image);
  if(imageAsset[2]){res.writeHead(200,{'content-type':'image/png','cache-control':'no-store'});return res.end(image.bytes);}
  return json(res,200,image.asset);
 }
 if(surface==='studio'&&url.pathname==='/api/v1/studio/imports'&&req.method==='POST'){
  const b=await body(req),bytes=Buffer.from(b.original_bytes),id='src_'+secret().slice(0,32);
  const record={id,imported_by:family.actor,recorded_at:'2026-10-18T00:00:00Z',sha256:createHash('sha256').update(bytes).digest('hex'),byte_len:bytes.length,metadata:b.metadata,outcome:{status:'parsed',extraction:{extractor_version:'loopback-mock-v1',blocks:[{index:0,kind:'paragraph',text:bytes.toString(),speaker:null,scene:null,cue_kind:null}],warnings:[],script_json:null}},original_text:bytes.toString()};
  state.imports.set(id,{record,bytes});return json(res,201,record);
 }
 if(surface==='studio'&&url.pathname.match(/^\/api\/v1\/studio\/imports\/src_[a-f0-9]{32}\/original$/)){
  const record=state.imports.get(url.pathname.split('/')[5]);if(!record)return error(res,404,'not_found');
  res.writeHead(200,{'content-type':'application/octet-stream','cache-control':'no-store'});return res.end(record.bytes);
 }
 if(suffix==='/session'&&url.pathname.startsWith(prefix)){
  const view=family.view;return json(res,200,{actor_id:family.actor,session_id:family.id,expires_at:view.expires_at,absolute_expires_at:view.absolute_expires_at,authenticated_at:view.authenticated_at});
 }
 if(url.pathname.includes('/library/')){if(state.holdLibrary)await state.holdLibrary;return json(res,200,{items:[],next_cursor:null});}
 return error(res,404,'not_found');
 }catch(_){error(res,400,'invalid_request');}
});
const issuer=http.createServer();
issuer.on('request',(req,res)=>{
 const u=new URL(req.url,issuerOrigin);if(u.pathname==='/select'){const f=state.flows.get(u.searchParams.get('flow'));if(f)f.actor=u.searchParams.get('actor')==='b'?actorB:actorA;res.writeHead(204);return res.end();}
 const id=u.searchParams.get('flow'),f=state.flows.get(id);if(!f){res.writeHead(404);return res.end();}
 const complete=`${origins[f.surface]}/auth/bearer/complete?flow_id=${f.id}&state=${f.state}`;
 res.writeHead(200,{'content-type':'text/html','referrer-policy':'no-referrer'});
 res.end(`<!doctype html><meta charset="utf-8"><title>MOCK issuer — no real identity</title><main><h1>Loopback mock issuer</h1><button id="mock-a">Synthetic A</button><button id="mock-b">Synthetic B</button><button id="mock-cancel">Cancel</button></main><script>const finish=async(actor)=>{await fetch('/select?flow=${f.id}&actor='+actor);location.href=${JSON.stringify(complete)}+'&code=${f.code}'};document.querySelector('#mock-a').onclick=()=>finish('a');document.querySelector('#mock-b').onclick=()=>finish('b');document.querySelector('#mock-cancel').onclick=()=>location.href=${JSON.stringify(complete+'&error=authorization_failed')};</script>`);
});
const servers=[api,issuer];
for(const [surface,origin]of Object.entries(origins)){
 const base=join(root,`apps/cantos-${surface}/web/dist`);
 servers.push(http.createServer(async(req,res)=>{try{
  const path=new URL(req.url,origin).pathname;const file=path==='/'||path==='/auth/bearer/complete'?'index.html':path.slice(1);
  if(file.includes('..'))throw new Error('path');const bytes=await readFile(join(base,file));const mime={'.html':'text/html','.js':'text/javascript','.wasm':'application/wasm','.css':'text/css','.svg':'image/svg+xml','.png':'image/png','.webp':'image/webp'}[extname(file)]||'application/octet-stream';
  res.writeHead(200,{'content-type':mime,'cache-control':'no-store','referrer-policy':'no-referrer'});res.end(bytes);
 }catch{res.writeHead(404);res.end();}}));
}
const ports=[18080,18083,18081,18082];let browser,ws,profile,browserVersion;let seq=0;const pending=new Map(),listeners=[];const pageErrors=[];listeners.push(message=>{if(message.method==='Runtime.exceptionThrown')pageErrors.push(message.params.exceptionDetails.text+': '+(message.params.exceptionDetails.exception?.description||''));});
const sleep=ms=>new Promise(r=>setTimeout(r,ms));
async function until(fn,label,timeout=15000){const start=Date.now();while(Date.now()-start<timeout){try{const value=await fn();if(value)return value;}catch{}await sleep(50);}throw new Error(`timeout: ${label}`);}
function send(method,params={},sessionId){const id=++seq;return new Promise((resolve,reject)=>{const timer=setTimeout(()=>{pending.delete(id);reject(new Error(`CDP timeout ${method}`));},15000);pending.set(id,{resolve,reject,timer});ws.send(JSON.stringify({id,method,params,...(sessionId?{sessionId}:{})}));});}
async function attach(targetId){const {sessionId}=await send('Target.attachToTarget',{targetId,flatten:true});await send('Runtime.enable',{},sessionId);await send('Page.enable',{},sessionId);await send('Network.enable',{},sessionId);return sessionId;}
async function evaluate(s,expression){const r=await send('Runtime.evaluate',{expression,returnByValue:true,awaitPromise:true},s);if(r.exceptionDetails)throw new Error('page evaluation failed');return r.result?.value;}
const visible=(s,selector)=>evaluate(s,`(()=>{const e=document.querySelector(${JSON.stringify(selector)});return !!(e&&e.getBoundingClientRect().height&&!e.closest('[hidden],[inert]'));})()`);
async function click(s,selector){await until(async()=>await visible(s,selector)&&await evaluate(s,`(()=>{const e=document.querySelector(${JSON.stringify(selector)});return !e.disabled&&e.getAttribute('aria-disabled')!=='true';})()`),`enabled ${selector}`);const p=await evaluate(s,`(()=>{const e=document.querySelector(${JSON.stringify(selector)});e.scrollIntoView({block:'center'});const r=e.getBoundingClientRect();return {x:r.x+r.width/2,y:r.y+r.height/2};})()`);for(const type of ['mousePressed','mouseReleased'])await send('Input.dispatchMouseEvent',{type,...p,button:'left',clickCount:1},s);}
async function page(url){const {targetId}=await send('Target.createTarget',{url});return {id:targetId,s:await attach(targetId)};}
async function popupFor(main,button){if(button==='#theatre-account-login'){await click(main.s,button);button='#theatre-auth-login';}const prior=new Set((await send('Target.getTargets')).targetInfos.map(t=>t.targetId));await click(main.s,button);const target=await until(async()=> (await send('Target.getTargets')).targetInfos.find(t=>t.type==='page'&&!prior.has(t.targetId)&&t.url.startsWith(issuerOrigin)),'issuer popup');return {id:target.targetId,s:await attach(target.targetId)};}
async function login(main,button,actor='a'){const popup=await popupFor(main,button);await click(popup.s,`#mock-${actor}`);return popup;}
async function checkStorage(main){const contents=await evaluate(main.s,'JSON.stringify([Object.entries(localStorage),Object.entries(sessionStorage)])');assert(!/ca1_[stn]_|cr1_|refresh_token|access_token/.test(contents));const url=await evaluate(main.s,'location.href');assert(!/ca1_|cr1_|access_token|refresh_token/.test(url));}
const downloads=new Map();listeners.push(message=>{if(message.method==='Browser.downloadProgress')downloads.set(message.params.guid,message.params.state);});
const scenarios=[];const passed=name=>{scenarios.push(name);console.log(`PASS ${name}`);};
try{
 for(let i=0;i<servers.length;i++)await new Promise((resolve,reject)=>{servers[i].once('error',reject);servers[i].listen(ports[i],'127.0.0.1',resolve);});
 profile=await mkdtemp(join(tmpdir(),'cantos-web-bearer-chromium-'));
 browser=spawn(chrome,['--headless=new',`--user-data-dir=${profile}`,'--remote-debugging-port=0','--remote-debugging-address=127.0.0.1','--no-first-run','--no-default-browser-check','--disable-background-networking','--disable-component-update','--disable-sync','--disable-extensions','--disable-crash-reporter','--no-proxy-server','--test-third-party-cookie-phaseout','about:blank'],{stdio:'ignore'});
 const port=await until(async()=>{const value=await readFile(join(profile,'DevToolsActivePort'),'utf8');return value.split('\n')[0];},'Chromium startup');
 const endpoint=await fetch(`http://127.0.0.1:${port}/json/version`).then(r=>r.json());
 ws=new WebSocket(endpoint.webSocketDebuggerUrl);await new Promise((r,j)=>{ws.onopen=r;ws.onerror=j;});
 ws.onmessage=event=>{const message=JSON.parse(event.data);if(message.id){const request=pending.get(message.id);if(request){pending.delete(message.id);clearTimeout(request.timer);message.error?request.reject(new Error(`CDP ${message.error.message}`)):request.resolve(message.result);}}else for(const listener of listeners)listener(message);};
 const version=await send('Browser.getVersion');browserVersion=version;
 const theatre=await page(`${origins.theatre}/#login?from=account`);
 await until(()=>visible(theatre.s,'#theatre-auth-login'),'Theatre guest');
 await send('Network.setCookieControls',{enableThirdPartyCookieRestriction:true,disableThirdPartyCookieMetadata:true,disableThirdPartyCookieHeuristics:true},theatre.s);
 const cookie=await send('Network.setCookie',{name:'synthetic_third_party',value:'fixture',url:apiOrigin,secure:true,sameSite:'None'},theatre.s);assert(cookie.success);
 const cookieControl=await page(apiOrigin+'/_test/cookie-control');await until(()=>evaluate(cookieControl.s,'document.body.innerText.includes("true")'),'first-party cookie positive control');
 const xhrCookie=await evaluate(cookieControl.s,"new Promise(resolve=>{const x=new XMLHttpRequest();x.open('GET','/_test/cookie-control');x.withCredentials=false;x.onload=()=>resolve(JSON.parse(x.responseText).cookie);x.send();})");assert.equal(xhrCookie,true);
 const fetchCookie=await evaluate(cookieControl.s,"fetch('/_test/cookie-control',{credentials:'omit'}).then(r=>r.json()).then(r=>r.cookie)");assert.equal(fetchCookie,false);passed('same-origin-xhr-cookie-positive-control-and-fetch-omit');await send('Target.closeTarget',{targetId:cookieControl.id});
 await evaluate(theatre.s,`fetch(${JSON.stringify(apiOrigin+'/_test/cookie-control')},{credentials:'include'}).then(r=>r.json())`);
 assert(state.cookieProbes.some(p=>p.origin===null&&p.cookie));assert(state.cookieProbes.some(p=>p.origin===origins.theatre&&!p.cookie));passed('third-party-cookie-blocking-with-first-party-positive-control');
 assert.equal(state.flows.size,0);await login(theatre,'#theatre-auth-login');
 await until(()=>visible(theatre.s,'#theatre-account-logout'),'Theatre API-confirmed account');
 assert.equal(state.exchanges,1);await until(()=>state.requests.some(r=>r.path.endsWith('/library/history')&&r.authorization),'Library bounded batch');
 await checkStorage(theatre);passed('theatre-popup-pkce-session-library-and-ram-only');
 // Actual reload loses document RAM and requires a deliberate authorization; never a guest loop.
 const flows=state.flows.size;await send('Page.reload',{},theatre.s);
 await until(()=>visible(theatre.s,'#theatre-account-login'),'Theatre reload guest');
 assert.equal(state.flows.size,flows);await checkStorage(theatre);passed('reload-requires-controlled-reauthorization-without-auto-loop');
 await login(theatre,'#theatre-account-login');await until(()=>visible(theatre.s,'#theatre-account-logout'),'login again');
 await click(theatre.s,'#theatre-account-logout');await until(()=>visible(theatre.s,'#theatre-account-login'),'logout ack');
 assert(state.revokes>0);passed('logout-revokes-family-before-new-login');
 const popup=await popupFor(theatre,'#theatre-account-login');await send('Target.closeTarget',{targetId:popup.id});
 await until(()=>evaluate(theatre.s,"document.querySelector('#theatre-auth-login')?.getAttribute('aria-disabled')!=='true'"),'closed popup retry');passed('popup-close-cancels-flow-and-allows-retry');
 state.shortAccess=true;state.dropRefresh=true;const refreshBefore=state.refreshes;
 await login(theatre,'#theatre-auth-login');
 await until(()=>visible(theatre.s,'#theatre-auth-refresh'),'unknown refresh failure');
 assert.equal(state.refreshes,refreshBefore+1);assert([...state.families.values()].filter(f=>f.surface==='theatre').at(-1).revoked);
 state.shortAccess=false;await click(theatre.s,'#theatre-auth-refresh');await until(()=>visible(theatre.s,'#theatre-auth-login'),'reauthorization after unknown refresh');
 assert.equal(state.refreshes,refreshBefore+1);passed('lost-refresh-response-revokes-family-without-replay');
 await send('Emulation.setDeviceMetricsOverride',{width:390,height:844,deviceScaleFactor:1,mobile:true},theatre.s);
 const image=await send('Page.captureScreenshot',{format:'png'},theatre.s);await writeFile(join(run,'theatre-guest-390-light-vi.png'),Buffer.from(image.data,'base64'));
 const studio=await page(`${origins.studio}/`);await send('Emulation.setDeviceMetricsOverride',{width:1440,height:960,deviceScaleFactor:1,mobile:false},studio.s);await until(()=>visible(studio.s,'#session-oidc'),'Studio login');
 await login(studio,'#session-oidc');await until(()=>visible(studio.s,'#workspace-import-control'),'Studio confirmed workspace');await checkStorage(studio);passed('studio-owner-bearer-and-no-cookie-fallback');
 await click(studio.s,'#workspace-dashboard-control');await click(studio.s,`#dashboard-work-${workId}`);
 await until(()=>visible(studio.s,'#artwork-file'),'private artwork owner form');
 const png=join(run,'synthetic.png');await writeFile(png,Buffer.from('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=','base64'));
 const {root:imageDom}=await send('DOM.getDocument',{},studio.s);const {nodeId:imageInput}=await send('DOM.querySelector',{nodeId:imageDom.nodeId,selector:'#artwork-file'},studio.s);await send('DOM.setFileInputFiles',{nodeId:imageInput,files:[png]},studio.s);
 for(const [selector,text]of [['#artwork-provenance','Ảnh tổng hợp dùng riêng cho fixture'],['#artwork-rights','synthetic_fixture'],['#artwork-alt','Một điểm sáng thử nghiệm']]){await click(studio.s,selector);await send('Input.insertText',{text},studio.s);}
 await click(studio.s,'#artwork-prepare');await click(studio.s,'#artwork-upload');await click(studio.s,'#artwork-reconcile');await click(studio.s,'#artwork-select');
 await until(()=>evaluate(studio.s,'(()=>{const e=document.querySelector(".artwork-current img");return !!(e&&e.src.startsWith("blob:")&&e.complete&&e.naturalWidth===1)})()'),'authenticated private artwork Blob preview');
 assert.equal(state.imagePuts,1);assert([...state.images.values()].every(image=>image.bytes?.equals(Buffer.from(readFileSync(png)))));
 assert(state.requests.some(r=>r.path.includes('/image-assets/')&&r.path.endsWith('/bytes')&&r.authorization&&!r.cookie));
 passed('artwork-xhr-unknown-upload-reconciles-once-and-private-preview-fetches-bearer');
 await click(studio.s,'#workspace-import-control');
 // Select a synthetic manuscript through the real file input, then retain metadata through reauth.
 const manuscript=join(run,'synthetic.txt');await writeFile(manuscript,'Ánh đèn cuối sân khấu\nNgày mai, mình có diễn tiếp không?\n');
 const {root:domRoot}=await send('DOM.getDocument',{},studio.s);const {nodeId}=await send('DOM.querySelector',{nodeId:domRoot.nodeId,selector:'#import-file'},studio.s);
 await send('DOM.setFileInputFiles',{nodeId,files:[manuscript]},studio.s);await until(()=>evaluate(studio.s,'document.querySelector("#import-selected-file").textContent==="synthetic.txt"'),'selected manuscript');
 await click(studio.s,'#import-reference');await send('Input.insertText',{text:'Bản nháp riêng của tài khoản A'},studio.s);
 assert.equal(await evaluate(studio.s,'document.querySelector("#import-reference").value'),'Bản nháp riêng của tài khoản A');
 await click(studio.s,'#workspace-revisions-control');const history=await send('Page.getNavigationHistory',{},studio.s);
 await send('Page.navigateToHistoryEntry',{entryId:history.entries[history.currentIndex-1].id},studio.s);await until(()=>visible(studio.s,'#import-reference'),'Back restores import pane');
 assert.equal(await evaluate(studio.s,'document.querySelector("#import-reference").value'),'Bản nháp riêng của tài khoản A');
 await send('Page.navigateToHistoryEntry',{entryId:history.entries[history.currentIndex].id},studio.s);await until(()=>evaluate(studio.s,'location.hash==="#workspace-revisions"'),'Forward restores pane');
 await send('Page.navigateToHistoryEntry',{entryId:history.entries[history.currentIndex-1].id},studio.s);await until(()=>visible(studio.s,'#import-reference'),'Back again');passed('studio-back-forward-keeps-owner-bound-draft');
 // Force admission failure; no new credential is silently adopted by a business request.
 state.denied=true;await click(studio.s,'summary');await click(studio.s,'#account-refresh');await until(()=>visible(studio.s,'#session-oidc'),'Studio reauth');state.denied=false;
 await login(studio,'#session-oidc','b');await until(()=>visible(studio.s,'#session-oidc'),'different account refused');
 assert(!await visible(studio.s,'#import-reference'));
 await login(studio,'#session-oidc','a');await until(()=>visible(studio.s,'#workspace-import-control'),'same owner restores');
 assert.equal(await evaluate(studio.s,'document.querySelector("#import-reference").value'),'Bản nháp riêng của tài khoản A');passed('studio-reauth-retains-draft-and-refuses-account-b');
 await send('Browser.setDownloadBehavior',{behavior:'allowAndName',downloadPath:run,eventsEnabled:true});
 await click(studio.s,'#workspace-import form:first-of-type button[type=submit]');
 await until(()=>evaluate(studio.s,'document.querySelector(".download-original button")!==null'),'stored source');
 await click(studio.s,'.download-original button');const guid=await until(()=>[...downloads.entries()].find(([_,state])=>state==='completed')?.[0],'authenticated original download');
 assert.deepEqual(await readFile(join(run,guid)),await readFile(manuscript));assert(state.requests.some(r=>r.path.endsWith('/original')&&r.authorization&&!r.cookie));passed('private-source-download-fetches-with-bearer-and-verifies-exact-bytes');
 // The owner must use a distinct real reauthentication intent; refresh is never that intent.
 if(!await evaluate(studio.s,'document.querySelector(".studio-account").open'))await click(studio.s,'summary');const fresh=await popupFor(studio,'#account-reauthenticate');
 assert([...state.flows.values()].at(-1).reauthenticate===true);await click(fresh.s,'#mock-a');await until(()=>visible(studio.s,'#workspace-import-control'),'fresh authentication return');passed('sensitive-reauthentication-has-a-distinct-pinned-owner-intent');
 let releaseRefresh;state.holdRefresh=new Promise(resolve=>releaseRefresh=resolve);state.shortAccess=true;const heldBefore=state.refreshes;
 if(!await evaluate(studio.s,'document.querySelector(".studio-account").open'))await click(studio.s,'summary');
 const interrupted=await popupFor(studio,'#account-reauthenticate');await click(interrupted.s,'#mock-a');await until(()=>state.refreshes===heldBefore+1,'refresh committed with held response');
 await click(studio.s,'#session-cancel');state.shortAccess=false;state.holdRefresh=null;
 const replacement=await popupFor(studio,'#session-oidc');const replacementFlow=[...state.flows.values()].at(-1);releaseRefresh();
 await evaluate(studio.s,'new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve)))');
 await click(replacement.s,'#mock-a');await until(()=>visible(studio.s,'#workspace-import-control'),'replacement survives old refresh');assert(!replacementFlow.cancelled);passed('cancelled-refresh-late-response-does-not-cancel-new-flow');
 await send('Emulation.setDeviceMetricsOverride',{width:1440,height:960,deviceScaleFactor:1,mobile:false},studio.s);
 const shot=await send('Page.captureScreenshot',{format:'png'},studio.s);await writeFile(join(run,'studio-draft-1440-light-vi.png'),Buffer.from(shot.data,'base64'));
 // A protected business read losing credentials must also clear the cached signed-in UI.
 await evaluate(studio.s,'window.__cantosOriginalNow=Date.now;Date.now=()=>window.__cantosOriginalNow()+110000');state.dropRefresh=true;
 await click(studio.s,'#workspace-dashboard-control');
 await until(()=>visible(studio.s,'#session-oidc'),'credential loss hides every private workspace');
 assert(!await visible(studio.s,'#workspace-dashboard-control'));
 await evaluate(studio.s,'Date.now=window.__cantosOriginalNow;delete window.__cantosOriginalNow');
 await login(studio,'#session-oidc');await until(()=>visible(studio.s,'#workspace-import-control'),'controlled recovery after background credential loss');
 passed('protected-read-refresh-loss-clears-cached-owner-projection');
 // Relevant narrow viewport, English/dark and CSS text scaling, with actual keyboard focus.
 if(!await evaluate(studio.s,'document.querySelector(".studio-account").open'))await click(studio.s,'summary');
 await click(studio.s,'#account-logout');await until(()=>visible(studio.s,'#session-oidc'),'Studio logout entry');
 await click(studio.s,'#studio-language');
 // Theme is a controlled DOM fixture event; native-select interaction is outside this claim.
 await evaluate(studio.s,'(()=>{const select=document.querySelector("#theme-select");select.value="dark";select.dispatchEvent(new Event("change",{bubbles:true}));})()');
 await until(()=>evaluate(studio.s,'document.querySelector("#theme-select").value==="dark"'),'controlled dark theme');
 await send('Emulation.setDeviceMetricsOverride',{width:390,height:844,deviceScaleFactor:1,mobile:true},studio.s);
 await evaluate(studio.s,'document.documentElement.style.fontSize="200%";document.querySelector("#theme-select").focus()');
 for(const type of ['keyDown','keyUp'])await send('Input.dispatchKeyEvent',{type,key:'Tab',code:'Tab',windowsVirtualKeyCode:9},studio.s);
 assert.equal(await evaluate(studio.s,'document.activeElement.id'),'session-oidc');
 assert.equal(await evaluate(studio.s,'document.activeElement.matches(":focus-visible")'),true);
 assert(await evaluate(studio.s,'document.documentElement.scrollWidth<=innerWidth'));
 const ax=await send('Accessibility.getFullAXTree',{},studio.s);assert(ax.nodes.some(n=>!n.ignored&&n.role?.value==='button'&&n.name?.value==='Sign in with Cantos ID'));
 const scaled=await send('Page.captureScreenshot',{format:'png'},studio.s);await writeFile(join(run,'studio-login-390-dark-en-css200-focus.png'),Buffer.from(scaled.data,'base64'));
 passed('studio-keyboard-focus-and-narrow-dark-english-css-text-200');
 assert(state.requests.every(r=>r.cookie===false));
 assert(state.requests.filter(r=>r.path.includes('/auth/bearer/')&&r.method==='POST').every(r=>!r.authorization));
 assert.deepEqual(source(),before,'source changed while browser evidence ran');
 const hashes={};for(const surface of ['studio','theatre'])hashes[surface]=createHash('sha256').update(await readFile(join(root,`apps/cantos-${surface}/web/dist/index.html`))).digest('hex');
 await writeFile(join(run,'report.json'),JSON.stringify({status:'PASS',source:before,browser:version,topology:{api:apiOrigin,web:origins,issuer:'loopback mock only'},scenarios,tests_passed:scenarios.length,third_party_cookies:'blocked by Network.setCookieControls and phaseout flag; cross-site credentialed probe omitted cookie while first-party positive control sent it',network:{requests:state.requests.length,cookies:0},bundles:hashes,realIssuer:'NOT_RUN'},null,2));
 console.log(`Evidence: ${run}`);
}catch(error){
 const pages=[];
 if(ws?.readyState===1){for(const target of (await send('Target.getTargets')).targetInfos.filter(t=>t.type==='page')){try{const session=await attach(target.targetId);pages.push({url:target.url.replace(/\?.*?(?=#|$)/,''),text:await evaluate(session,'document.body.innerText')});}catch{}}}
 await writeFile(join(run,'failure.json'),JSON.stringify({status:'FAIL',source:before,browser:browserVersion,scenarios,error:String(error.message),requests:state.requests,counts:{flows:state.flows.size,exchanges:state.exchanges,refreshes:state.refreshes,cancels:state.cancels,revokes:state.revokes},pages,pageErrors},null,2));
 console.error(`FAIL ${error.message}; evidence ${run}`);process.exitCode=1;
}finally{
 if(ws?.readyState===1){await send('Browser.close').catch(()=>{});ws.close();}
 if(browser&&!browser.killed)browser.kill();for(const server of servers)server.closeAllConnections();for(const server of servers)server.close();
 if(profile)await rm(profile,{recursive:true,force:true}).catch(()=>{});
 for(const request of pending.values()){clearTimeout(request.timer);request.reject(new Error('fixture shutdown'));}pending.clear();
}
