module.exports = {
    checkString: function(val, name) {
        if (typeof val === 'string' || val instanceof String){
            return val;
        } else {
            throw new TypeError(`${name} should be a string, but '${typeof val}' given`);
        }
    },
    checkBool: function(val, name) {
        if(typeof val === 'boolean') {
            return val;
        } else {
            throw new TypeError(`${name} should be boolean, but ${typeof val} given`);
        }
    }
}