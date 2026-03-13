import http from 'k6/http';
import { check, sleep } from 'k6';
import { uuidv4 } from 'https://jslib.k6.io/k6-utils/1.4.0/index.js';

export const options = {
    scenarios: {
        // 1. Read Path: 10.000 RPS
        read_path: {
            executor: 'constant-arrival-rate',
            rate: 10000,
            timeUnit: '1s',
            duration: '1m',
            preAllocatedVUs: 200,
            maxVUs: 1000,
            exec: 'readTest',
        },
        // 2. Batch Path: 1.000 RPS
        batch_path: {
            executor: 'constant-arrival-rate',
            rate: 1000,
            timeUnit: '1s',
            duration: '1m',
            preAllocatedVUs: 50,
            maxVUs: 200,
            exec: 'batchTest',
        },
        // 3. Write Path: 500 RPS
        write_path: {
            executor: 'constant-arrival-rate',
            rate: 500,
            timeUnit: '1s',
            duration: '1m',
            preAllocatedVUs: 20,
            maxVUs: 100,
            exec: 'writeTest',
        },
        // 4. Mixed Path (80/15/5 ratio)
        mixed_path: {
            executor: 'ramping-arrival-rate',
            startRate: 100,
            timeUnit: '1s',
            stages: [
                { target: 2000, duration: '30s' }, // Warmup
                { target: 2000, duration: '1m' },  // Plateau
            ],
            preAllocatedVUs: 100,
            exec: 'mixedTest',
        },
    },

    thresholds: {
        'http_req_duration{scenario:read_path}': ['p(99)<10'],
        'http_req_duration{scenario:batch_path}': ['p(99)<50'],
        'http_req_duration{scenario:write_path}': ['p(99)<100'],
        'http_req_duration{scenario:mixed_path}': ['p(99)<100'], 
        'http_req_failed': ['rate<0.01'],
    },
};

const BASE_URL = __ENV.BASE_URL || 'http://localhost:8080';
const CONTENT_TYPES = ['post', 'bonus_hunter', 'top_picks'];

function getRandomItem() {
    return {
        type: CONTENT_TYPES[Math.floor(Math.random() * CONTENT_TYPES.length)],
        id: "731b0395-4888-4822-b516-05b4b7bf2089" 
    };
}


export function readTest() {
    const item = getRandomItem();
    const res = http.get(`${BASE_URL}/v1/likes/${item.type}/${item.id}/count`, {
        headers: { 'x-request-id': uuidv4() }
    });
    check(res, { 'status is 200': (r) => r.status === 200 });
}

export function writeTest() {
    const item = getRandomItem();
    const payload = JSON.stringify({
        content_type: item.type,
        content_id: item.id,
    });
    const params = {
        headers: {
            'Content-Type': 'application/json',
            'Authorization': 'Bearer 550e8400-e29b-41d4-a716-446655440001',
            'x-request-id': uuidv4(),
        },
    };

    const res = http.post(`${BASE_URL}/v1/likes`, payload, params);
    check(res, { 'status is 200 or 201': (r) => r.status === 200 || r.status === 201 });
}

export function batchTest() {
    const items = [];
    for (let i = 0; i < 50; i++) {
        items.push(getRandomItem());
    }

    const payload = JSON.stringify({
        items: items.map(i => ({
            content_type: i.type,
            content_id: i.id
        }))
    });

    const res = http.post(`${BASE_URL}/v1/likes/batch/counts`, payload, {
        headers: { 'Content-Type': 'application/json', 'x-request-id': uuidv4() }
    });
    check(res, { 'batch status 200': (r) => r.status === 200 });
}

export function mixedTest() {
    const rand = Math.random();
    if (rand < 0.80) {
        readTest();
    } else if (rand < 0.95) {
        batchTest();
    } else {
        writeTest();
    }
}
