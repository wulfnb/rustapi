import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import vm from 'node:vm';
import test from 'node:test';
const source = await readFile(new URL('../browser-connector/popup.js',import.meta.url),'utf8');
async function popup(direction, supplied, pairValue='http://127.0.0.1:1234/#'+'a'.repeat(64)) {
  const nodes=new Map(['pair','status','capture','sync'].map(id=>[id,{value:'',textContent:'',addEventListener(_,callback){this.click=callback;}}]));
  const calls=[], written=[];
  const chrome={storage:{session:{async get(){return {pair:pairValue};},async set(){}}},tabs:{async query(){return [{url:'https://api.example.test/v1'}];}},permissions:{async request(){return true;}},cookies:{async getAll(){return supplied;},async set(value){written.push(value);return value;}}};
  const fetch=async (url,options)=>{calls.push({url,options});return {ok:true,async json(){return direction==='capture'?{count:1}:{cookies:supplied};}};};
  await vm.runInNewContext(`(async()=>{${source}})()`,{document:{getElementById:id=>nodes.get(id)},chrome,fetch,URL,AbortSignal});
  nodes.get(direction).click();
  for(let attempt=0;attempt<50&&!/captured|merged|pairing URL|failed/.test(nodes.get('status').textContent);attempt++)await new Promise(resolve=>setTimeout(resolve,2));
  return {nodes,calls,written};
}
test('capture sends matching unpartitioned host cookies with pairing authorization',async()=>{
  const result=await popup('capture',[
    {name:'one',domain:'api.example.test',hostOnly:true},
    {name:'parent',domain:'.example.test',hostOnly:false},
    {name:'child',domain:'child.api.example.test',hostOnly:true},
    {name:'partition',domain:'api.example.test',hostOnly:true,partitionKey:{}},
  ]);
  assert.equal(result.calls.length,1);
  assert.equal(result.calls[0].options.headers.Authorization,'Bearer '+'a'.repeat(64));
  assert.deepEqual(JSON.parse(result.calls[0].options.body).cookies.map(c=>c.name),['one','parent']);
  assert.match(result.nodes.get('status').textContent,/partitioned cookies skipped/);
});
test('merge preserves security/path/session attributes and maps SameSite None',async()=>{
  const result=await popup('sync',[{name:'session',value:'private-test',domain:'api.example.test',path:'/v1',hostOnly:true,secure:true,httpOnly:true,sameSite:'none',expirationDate:null}]);
  assert.equal(result.written.length,1);const value=result.written[0];
  assert.equal(value.url,'https://api.example.test/v1');assert.equal(value.sameSite,'no_restriction');assert.equal(value.httpOnly,true);assert.equal(value.domain,undefined);assert.equal(value.expirationDate,undefined);
});
test('pairing cannot redirect cookie data to a remote server',async()=>{
  const result=await popup('capture',[],'https://evil.test/#'+'a'.repeat(64));assert.equal(result.calls.length,0);assert.match(result.nodes.get('status').textContent,/pairing URL/);
});
