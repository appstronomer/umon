const validator = require('./validator');

const ENUM_TYPE = {
    f: 'offline',
    n: 'online',
    v: 'value',
}

function checkFunc(val, name) {
    if (typeof val === 'function'){
        return val;
    } else {
        throw new TypeError(`${name} should be a function, but ${typeof val} given`);
    }
}

function onMessage(self, func) {
    self.onMessage = checkFunc(func, 'onMessage');
}

function onConnect(self, func) {
    self.onConnect = checkFunc(func, 'onConnect');
}

function onDisconnect(self, func) {
    self.onDisconnect = checkFunc(func, 'onDisconnect');
}

function tick(self, socket) {
    const pingValLast = self.pingVal;
    setTimeout(() => {
        if(pingValLast === self.pingVal) {
            socket.close(4102, 'ping not received in time');
        }
    }, self.tick);
}

function getGroups(self) {
    const objGroups = {};
    const sessGroups = self.sess.groups;
    for(const groupName in sessGroups) if(sessGroups.hasOwnProperty(groupName)) {
        const units = sessGroups[groupName];
        const arrUnit = [];
        for(const unitName in units) if(units.hasOwnProperty(unitName)) {
            arrUnit.push(unitName);
        }
        objGroups[groupName] = arrUnit;
    }
    return objGroups;
}

function processMessage(self, valueArr) {
    valueArr.forEach(data => {
        /*
        g: "local-group"
        i: 14
        t: 1660565313577
        u: "unit002"
        v: "b2s="
        y: "v"
        */
        let dto = {
            group: data.g,
            unit: data.u,
            idx: data.i,
            time: data.t,
            type: ENUM_TYPE[data.y],
        };
        if(typeof data.v === 'string') dto.value = data.v;
        // TODO: check monotonic consistency
        /*
        ivl: [],
        idx: null,
        type: null,
        time: null,
        val: null,
         */
        const groupObj = self.sess.groups[dto.group];
        if(groupObj) {
            const unitObj = groupObj[dto.unit];
            if(unitObj) {
                if(unitObj.idx === null || dto.idx === unitObj.idx + 1) {
                    unitObj.idx = dto.idx;
                    unitObj.type = dto.type;
                    unitObj.time = dto.time;
                    unitObj.val = dto.val;
                } else if(dto.idx > unitObj.idx + 1) {
                    self.hist.ivl.push({
                        group: dto.group,
                        unit: dto.unit,
                        min: unitObj.idx + 1,
                        max: dto.idx - 1,
                    });
                    unitObj.idx = dto.idx;
                    unitObj.type = dto.type;
                    unitObj.time = dto.time;
                    unitObj.val = dto.val;
                }
            }
        }
        self.onMessage(dto);
        if(self.hist.ivl.length > 0 && !self.hist.isActive) {
            histStep(self).then(()=>{});
        }
    })
}

async function histStep(self) {
    self.hist.isActive = true;
    let range = self.hist.ivl.pop();
    while(range) {
        if(!await histProcess(self, range)) { break; }
        range = self.hist.ivl.pop();
    }
    self.hist.isActive = false;
}
async function histProcess(self, range) {
    while(true) {
        range.div = range.max - range.min < 100 ? range.min : range.max - 99;
        while(true) {
            const res = await fetch(self.sess.path.hist + `?i=${encodeURIComponent(range.div)}&a=${range.max}&g=${encodeURIComponent(range.group)}&u=${encodeURIComponent(range.unit)}`, {
                mode: 'cors',
                redirect: 'follow',
                headers: {'sess': self.sess.token},
            });
            if(res.status === 200) {
                const resArr = (await res.json()).map(data => {
                    data.g = range.group;
                    data.u = range.unit;
                    return data;
                });
                processMessage(self, resArr);
                break;
            } else if(res.status === 401) {
                self.promise.rej(new Error('Unauthorized'));
                return false;
            }
        }
        if(range.div !== range.min) {
            range.max = range.div - 1;
        } else {
            return true;
        }
    }
}

function processConnection(self, map) {
        const resArr = [];
        for(const groupName in map) if(map.hasOwnProperty(groupName)) {
            const unitArray = map[groupName];
            unitArray.forEach(([unitName, data]) => {
                if(data) {
                    data.g = groupName;
                    data.u = unitName;
                    resArr.push(data);
                }
            })
        }
        processMessage(self, resArr);
}

function connMake(self) {
    const socket = new WebSocket(self.sess.path.ws);
    const handleOpen = () => {
        socket.send(self.sess.token);
        tick(self, socket);
    };
    const handleClose = evt => {
        if(evt.code === 3000) {
            self.promise.rej(new Error('Unauthorized'));
        } else {
            if(self.isConnected) {
                self.isConnected = false;
                self.onDisconnect(evt);
            }
            socket.removeEventListener('open', handleOpen);
            socket.removeEventListener('close', handleClose);
            socket.removeEventListener('message', handleMessage);
            setTimeout(() => connMake(self), 1000);
        }
    };
    const handleMessage = evt => {
        if(self.testLoose) return;
        try {
            const obj = JSON.parse(evt.data);
            if(obj.x === 'c') {
                console.log('[CONNECTED]:', obj);
                if(!self.isConnected) {
                    self.pingVal = 0;
                    self.isConnected = true;
                    self.onConnect();
                }
                if(typeof obj.m === 'object') {
                    processConnection(self, obj.m);
                }
            } else if(obj.x === 'p') {
                if(self.pingVal === obj.v) {
                    self.pingVal += 1;
                    socket.send(JSON.stringify({t: 'p', v: obj.v}));
                    tick(self, socket);
                } else {
                    socket.close(4102, 'incorrect ping value');
                }
            } else if(obj.x === 'd') {
                processMessage(self, obj.d);
            }
        } catch (e) {
            socket.close(4102, e.message);
        }
    };
    // TODO: establish last: fetch and push values to 'handleMessage'!
    // TODO: interval checker should be done inside of 'handleMessage': idx=null, idx=newIdx, idx<newIdx-1, idx>newIdx

    socket.addEventListener('open', handleOpen);
    socket.addEventListener('close', handleClose);
    socket.addEventListener('message', handleMessage);
}

async function serve(self) {
    const servePromise = new Promise((resArg, rejArg) => {
        self.promise.res = resArg;
        self.promise.rej = rejArg;
    });
    connMake(self);
    return servePromise;
}

module.exports = function (sess) {
    const self = {
        promise: {
            res: null,
            rej: null,
        },
        sess: sess,
        onConnect: null,
        onDisconnect: null,
        onMessage: null,
        pingVal: 0,
        isConnected: false,
        tick: 10000,
        hist: {
            isActive: false,
            ivl: [],
        },
        testLoose: false,
    }
    return {
        getToken: () => {return self.sess.token},
        testLoose: isLoose => self.testLoose = isLoose,
        getGroups: () => getGroups(self),
        onMessage: func => onMessage(self, func),
        onConnect: func => onConnect(self, func),
        onDisconnect: func => onDisconnect(self, func),
        serve: async () => serve(self),
    }
}