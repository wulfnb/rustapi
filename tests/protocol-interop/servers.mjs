import net from 'node:net';
import http from 'node:http';
import tls from 'node:tls';
import fs from 'node:fs';
import {createRequire} from 'node:module';
import {fileURLToPath} from 'node:url';
const require=createRequire(process.env.RUSTAPI_PROTOCOL_NODE_MODULES+'/package.json');
const {Aedes}=await import(require.resolve('aedes'));
const {Server}=require('socket.io');
const grpc=require('@grpc/grpc-js');
const loader=require('@grpc/proto-loader');
const broker=await Aedes.createBroker();
const mqtt=net.createServer(broker.handle);
await new Promise(resolve=>mqtt.listen(0,'127.0.0.1',resolve));
const mqtts=tls.createServer({key:fs.readFileSync(process.env.RUSTAPI_TEST_TLS_KEY),cert:fs.readFileSync(process.env.RUSTAPI_TEST_TLS_CERT)},broker.handle);
mqtts.on('tlsClientError',()=>{});
await new Promise(resolve=>mqtts.listen(0,'127.0.0.1',resolve));
const web=http.createServer();
const io=new Server(web,{transports:['websocket'],pingInterval:100,pingTimeout:1000});
io.use((socket,next)=>socket.handshake.auth.token==='validator-token'?next():next(new Error('bad token')));
io.on('connection',socket=>socket.on('echo',data=>socket.emit('reply',{received:data})));
await new Promise(resolve=>web.listen(0,'127.0.0.1',resolve));
const definition=loader.loadSync(fileURLToPath(new URL('./echo.proto',import.meta.url)),{});
const fixture=grpc.loadPackageDefinition(definition).fixture;
const rpc=new grpc.Server();
rpc.addService(fixture.Echo.service,{
  Unary(call,callback){if(call.metadata.get('authorization')[0]!=='Bearer validator-token')return callback({code:grpc.status.UNAUTHENTICATED});callback(null,{text:call.request.text});},
  Stream(call){for(let i=0;i<3;i++)call.write({text:call.request.text+'-'+i});call.end();},
});
const port=await new Promise((resolve,reject)=>rpc.bindAsync('127.0.0.1:0',grpc.ServerCredentials.createInsecure(),(error,port)=>error?reject(error):resolve(port)));
console.log(JSON.stringify({mqtt:mqtt.address().port,mqtts:mqtts.address().port,socket:web.address().port,grpc:port}));
const stop=()=>{io.close();mqtt.close();mqtts.close();broker.close();rpc.forceShutdown();process.exit(0);};
process.on('SIGTERM',stop);process.on('SIGINT',stop);
