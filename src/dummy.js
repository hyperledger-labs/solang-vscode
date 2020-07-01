const { cpuUsage } = require("process");

let a;
a = () => process.stdin.pipe(process.stdout);
a();
/*
while(true){
    console.log(i);
    i=i+1;
    console.log(a());
}
*/

const fs = require('fs');

const chld_proc = require('child_process');

var som = chld_proc.spawn('cargo run',['--example', 'goto_def'], {cwd:'/home/hyperion/intern/hyperledger/sls/practice/lsp-server', shell:true});
console.log(som);