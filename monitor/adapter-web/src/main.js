const sessionBuilder = require('./session');

if(typeof module === "object" && typeof module.exports === "object") {
    module.exports = sessionBuilder;
}
if(typeof window === "object"){
    window.MQTTMonitor = sessionBuilder;
}

console.log('new test 8')