const sessionBuild = require('./adapter');
const validator = require('./validator');


async function reqSessionMake(path, login, password) {
    const response = await fetch(path, {
        method: 'POST',
        mode: 'cors',
        headers: {'Content-Type': 'application/json'},
        body: JSON.stringify({'l': login, 'p': password}),
    });
    if(response.status === 200){
        return await response.text();
    }else if(response.status === 401) {
        throw new Error(`wrong credentials for '${login}' login`);
    } else {
        throw new Error(`http error status=${response.status}; text=${response.statusText}`);
    }
}

async function reqWplaceGet(path, token) {
    const res = await fetch(path, {
        mode: 'cors',
        headers: {'sess': token},
    });
    if(res.status !== 200) { throw new Error(`wrong wplace response status=${res.status}, statusText=${res.statusText}`); }
    let groupsResp = await res.json();
    const objGroups = {};
    for(const groupName in groupsResp) if(groupsResp.hasOwnProperty(groupName)) {
        const unitArr = groupsResp[groupName];
        const objUnits = {};
        unitArr.forEach(unit => objUnits[unit] = {
            idx: null,
            type: null,
            time: null,
            val: null,
        });
        objGroups[groupName] = objUnits;
    }
    return objGroups;
}

async function finalize(self) {
    const protocolWs = self.isSecure ? 'wss://' : 'ws://';
    const protocolHttp = self.isSecure ? 'https://' : 'http://';
    self.path = {
        login: `${protocolHttp}${self.uri}/login`,
        ws: `${protocolWs}${self.uri}/ws`,
        wplace: `${protocolHttp}${self.uri}/wplace`,
        wplaceLast: `${protocolHttp}${self.uri}/wplace-last`,
        hist: `${protocolHttp}${self.uri}/hist`,
    };
    if(!self.token) {
        if(!self.credentials) { throw new Error('one of the following must be specified: token by passing to builder.session() or login/password by passing to builder.credentials()'); }
        const password = self.credentials.password;
        self.password = null;
        self.token = await reqSessionMake(self.path.login, self.credentials.login, password);
        self.groups = await reqWplaceGet(self.path.wplace, self.token);
    } else {
        try {
            self.groups = await reqWplaceGet(self.path.wplace, self.token);
        } catch (e) {
            if(self.credentials) {
                const password = self.password;
                self.password = null;
                self.token = await reqSessionMake(self.path.login, self.login, password);
                self.groups = await reqWplaceGet(self.path.wplace, self.token);
            } else {
                throw e;
            }
        }
    }
    return sessionBuild(self);
}

function session(self, token) {
    self.token = validator.checkString(token, 'session token');
}

function credentials(self, login, password) {
    const loginChecked = validator.checkString(login, 'login');
    const passwordChecked = validator.checkString(password, 'password');
    self.credentials = {
        login: loginChecked,
        password: passwordChecked
    };
}

function isSecure(self, isSec) {
    self.isSecure = validator.checkBool(isSec, 'isSecure');
}



module.exports = function (uri) {
    const reUrl = /^(?:(?:(?:(?:[0-9]|[1-9][0-9]|1[0-9]{2}|2[0-4][0-9]|25[0-5])\.){3}(?:[0-9]|[1-9][0-9]|1[0-9]{2}|2[0-4][0-9]|25[0-5]))|(?:(?:(?:[a-zA-Z0-9]|[a-zA-Z0-9][a-zA-Z0-9\-]*[a-zA-Z0-9])\.)*(?:[A-Za-z0-9]|[A-Za-z0-9][A-Za-z0-9\-]*[A-Za-z0-9]))):(?:(?:6553[0-5])|(?:655[0-2][0-9])|(?:65[0-4][0-9]{2})|(?:6[0-4][0-9]{3})|(?:[1-5][0-9]{4})|(?:[0-5]{1,5})|(?:[0-9]{1,4}))(\/((([a-z]|\d|-|\.|_|~|[\u00A0-\uD7FF\uF900-\uFDCF\uFDF0-\uFFEF])|(%[\da-f]{2}))+(\/(([a-z]|\d|-|\.|_|~|[\u00A0-\uD7FF\uF900-\uFDCF\uFDF0-\uFFEF])|(%[\da-f]{2}))*)*)?)?$/mi;
    if (!reUrl.test(uri)) {
        throw new Error(`uri should consist of host, port and path without scheme, but '${uri}' given'`);
    }
    const self = {
        uri: '/' === uri.slice(-1) ? uri.slice(0, -1) : uri,
        path: null,
        token: null,
        credentials: null,
        password: null,
        isSecure: true,
        groups: null,
    };
    const builder = {
        credentials: (login, password) => { credentials(self, login, password); return builder; },
        session: (token) => { session(self, token); return builder; },
        isSecure: (isSec) => { isSecure(self, isSec); return builder; },
        finalize: async () => finalize(self),
    };
    return builder;
}
