const path = require('path');

module.exports = (env) => {
    const targtPath = env.target === 'volume' ? '../../volume-dst/storage/public' : 'dst/'
    return {
        mode: 'production',
        entry: './src/main.js',
        output: {
            filename: 'MQTTMonitor.js',
            path: path.resolve(__dirname, targtPath),
        },
    }
};