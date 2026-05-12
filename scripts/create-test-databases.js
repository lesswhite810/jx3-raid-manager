import fs from 'fs';
import path from 'path';
import { execSync } from 'child_process';
import { fileURLToPath } from 'url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const TestDir = path.join(process.env.APPDATA, 'com.jx3raidmanager.app', 'upgrade-tests');

function ensureDir(dir) {
  if (!fs.existsSync(dir)) {
    fs.mkdirSync(dir, { recursive: true });
  }
}

const BASE_TABLES = `
CREATE TABLE config (key TEXT PRIMARY KEY, value TEXT);
CREATE TABLE cache (key TEXT PRIMARY KEY, value TEXT, updated_at TEXT);
CREATE TABLE daily_records (id TEXT PRIMARY KEY, data TEXT, updated_at TEXT);
`;

function createDatabase(dbPath, sql) {
  if (fs.existsSync(dbPath)) fs.unlinkSync(dbPath);
  fs.writeFileSync(dbPath + '.sql', sql, 'utf8');
  execSync(`sqlite3 "${dbPath}" < "${dbPath}.sql"`, { encoding: 'utf8' });
  fs.unlinkSync(dbPath + '.sql');
}

function createV1Database(dbPath) {
  const sql = `
CREATE TABLE schema_versions (version INTEGER PRIMARY KEY, applied_at TEXT NOT NULL, description TEXT);
INSERT INTO schema_versions VALUES (1, '2024-01-01T00:00:00Z', 'V1');
CREATE TABLE accounts (id TEXT PRIMARY KEY, account_name TEXT NOT NULL, account_type TEXT NOT NULL DEFAULT 'OWN', sort_order INTEGER DEFAULT 0, password TEXT, notes TEXT, hidden INTEGER DEFAULT 0, disabled INTEGER DEFAULT 0, created_at TEXT, updated_at TEXT);
CREATE TABLE roles (id TEXT PRIMARY KEY, account_id TEXT NOT NULL, name TEXT NOT NULL, server TEXT, region TEXT, sect TEXT, martial TEXT, equipment_score INTEGER, disabled INTEGER DEFAULT 0, created_at TEXT, updated_at TEXT);
${BASE_TABLES}
INSERT INTO accounts (id, account_name, account_type, created_at, updated_at) VALUES ('test-acc-1', '测试账号1', 'OWN', '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z');
INSERT INTO roles (id, account_id, name, server, sect, created_at, updated_at) VALUES ('role-1', 'test-acc-1', '测试角色1', '电信五区', '少林', '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z');
`;
  createDatabase(dbPath, sql);
  console.log(`V1 数据库创建成功: ${dbPath}`);
}

function createV2Database(dbPath) {
  const sql = `
CREATE TABLE schema_versions (version INTEGER PRIMARY KEY, applied_at TEXT NOT NULL, description TEXT);
INSERT INTO schema_versions VALUES (1, '2024-01-01T00:00:00Z', 'V1'), (2, '2024-01-02T00:00:00Z', 'V2');
CREATE TABLE accounts (id TEXT PRIMARY KEY, account_name TEXT NOT NULL, account_type TEXT NOT NULL DEFAULT 'OWN', sort_order INTEGER DEFAULT 0, password TEXT, notes TEXT, hidden INTEGER DEFAULT 0, disabled INTEGER DEFAULT 0, created_at TEXT, updated_at TEXT);
CREATE TABLE roles (id TEXT PRIMARY KEY, account_id TEXT NOT NULL, name TEXT NOT NULL, server TEXT, region TEXT, sect TEXT, martial TEXT, equipment_score INTEGER, disabled INTEGER DEFAULT 0, created_at TEXT, updated_at TEXT);
CREATE TABLE raids (id TEXT PRIMARY KEY, name TEXT NOT NULL, difficulty TEXT NOT NULL DEFAULT '普通', player_count INTEGER NOT NULL DEFAULT 25, version TEXT, notes TEXT, is_active INTEGER DEFAULT 1, is_static INTEGER DEFAULT 0);
CREATE TABLE raid_bosses (id TEXT PRIMARY KEY, raid_name TEXT NOT NULL, name TEXT NOT NULL, boss_order INTEGER NOT NULL);
CREATE TABLE raid_versions (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT UNIQUE NOT NULL);
${BASE_TABLES}
INSERT INTO accounts (id, account_name, account_type, created_at, updated_at) VALUES ('test-acc-1', '测试账号1', 'OWN', '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z');
INSERT INTO roles (id, account_id, name, server, sect, created_at, updated_at) VALUES ('role-1', 'test-acc-1', '测试角色1', '电信五区', '少林', '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z');
INSERT INTO raids (id, name, difficulty, player_count, version, is_static) VALUES ('25人普通达摩洞', '达摩洞', '普通', 25, '世外蓬莱', 1);
`;
  createDatabase(dbPath, sql);
  console.log(`V2 数据库创建成功: ${dbPath}`);
}

function createV3Database(dbPath) {
  const sql = `
CREATE TABLE schema_versions (version INTEGER PRIMARY KEY, applied_at TEXT NOT NULL, description TEXT);
INSERT INTO schema_versions VALUES (1, '2024-01-01T00:00:00Z', 'V1'), (2, '2024-01-02T00:00:00Z', 'V2'), (3, '2024-01-03T00:00:00Z', 'V3');
CREATE TABLE accounts (id TEXT PRIMARY KEY, account_name TEXT NOT NULL, account_type TEXT NOT NULL DEFAULT 'OWN', sort_order INTEGER DEFAULT 0, password TEXT, notes TEXT, hidden INTEGER DEFAULT 0, disabled INTEGER DEFAULT 0, created_at TEXT, updated_at TEXT);
CREATE TABLE roles (id TEXT PRIMARY KEY, account_id TEXT NOT NULL, name TEXT NOT NULL, server TEXT, region TEXT, sect TEXT, martial TEXT, equipment_score INTEGER, disabled INTEGER DEFAULT 0, created_at TEXT, updated_at TEXT);
CREATE TABLE raids (id TEXT PRIMARY KEY, name TEXT NOT NULL, difficulty TEXT NOT NULL DEFAULT '普通', player_count INTEGER NOT NULL DEFAULT 25, version TEXT, notes TEXT, is_active INTEGER DEFAULT 1, is_static INTEGER DEFAULT 0);
CREATE TABLE raid_bosses (id TEXT PRIMARY KEY, raid_name TEXT NOT NULL, name TEXT NOT NULL, boss_order INTEGER NOT NULL);
CREATE TABLE raid_versions (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT UNIQUE NOT NULL);
CREATE TABLE trial_records (id TEXT PRIMARY KEY, account_id TEXT, role_id TEXT, role_name TEXT DEFAULT '', server TEXT DEFAULT '', layer INTEGER, bosses TEXT, card_1 TEXT, card_2 TEXT, card_3 TEXT, card_4 TEXT, card_5 TEXT, flipped_index INTEGER, record_type TEXT DEFAULT 'trial', date INTEGER NOT NULL, notes TEXT, updated_at TEXT);
CREATE TABLE baizhan_records (id TEXT PRIMARY KEY, account_id TEXT NOT NULL, role_id TEXT NOT NULL, role_name TEXT, server TEXT, date INTEGER NOT NULL, gold_income INTEGER DEFAULT 0, gold_expense INTEGER DEFAULT 0, notes TEXT, record_type TEXT DEFAULT 'baizhan', updated_at TEXT);
${BASE_TABLES}
INSERT INTO accounts (id, account_name, account_type, created_at, updated_at) VALUES ('test-acc-1', '测试账号1', 'OWN', '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z');
INSERT INTO roles (id, account_id, name, server, sect, created_at, updated_at) VALUES ('role-1', 'test-acc-1', '测试角色1', '电信五区', '少林', '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z');
INSERT INTO trial_records (id, account_id, role_id, layer, date) VALUES ('trial-1', 'test-acc-1', 'role-1', 50, 1704067200000);
INSERT INTO baizhan_records (id, account_id, role_id, date, gold_income) VALUES ('baizhan-1', 'test-acc-1', 'role-1', 1704067200000, 10000);
`;
  createDatabase(dbPath, sql);
  console.log(`V3 数据库创建成功: ${dbPath}`);
}

function createV4Database(dbPath) {
  const sql = `
CREATE TABLE schema_versions (version INTEGER PRIMARY KEY, applied_at TEXT NOT NULL, description TEXT);
INSERT INTO schema_versions VALUES (1, '2024-01-01T00:00:00Z', 'V1'), (2, '2024-01-02T00:00:00Z', 'V2'), (3, '2024-01-03T00:00:00Z', 'V3'), (4, '2024-01-04T00:00:00Z', 'V4');
CREATE TABLE accounts (id TEXT PRIMARY KEY, account_name TEXT NOT NULL, account_type TEXT NOT NULL DEFAULT 'OWN', sort_order INTEGER DEFAULT 0, password TEXT, notes TEXT, hidden INTEGER DEFAULT 0, disabled INTEGER DEFAULT 0, created_at TEXT, updated_at TEXT);
CREATE TABLE roles (id TEXT PRIMARY KEY, account_id TEXT NOT NULL, name TEXT NOT NULL, server TEXT, region TEXT, sect TEXT, martial TEXT, equipment_score INTEGER, disabled INTEGER DEFAULT 0, created_at TEXT, updated_at TEXT);
CREATE TABLE raids (id TEXT PRIMARY KEY, name TEXT NOT NULL, difficulty TEXT NOT NULL DEFAULT '普通', player_count INTEGER NOT NULL DEFAULT 25, version TEXT, notes TEXT, is_active INTEGER DEFAULT 1, is_static INTEGER DEFAULT 0);
CREATE TABLE raid_bosses (id TEXT PRIMARY KEY, raid_name TEXT NOT NULL, name TEXT NOT NULL, boss_order INTEGER NOT NULL);
CREATE TABLE raid_versions (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT UNIQUE NOT NULL);
CREATE TABLE trial_records (id TEXT PRIMARY KEY, account_id TEXT, role_id TEXT, role_name TEXT DEFAULT '', server TEXT DEFAULT '', layer INTEGER, bosses TEXT, card_1 TEXT, card_2 TEXT, card_3 TEXT, card_4 TEXT, card_5 TEXT, flipped_index INTEGER, record_type TEXT DEFAULT 'trial', date INTEGER NOT NULL, notes TEXT, updated_at TEXT);
CREATE TABLE baizhan_records (id TEXT PRIMARY KEY, account_id TEXT NOT NULL, role_id TEXT NOT NULL, role_name TEXT, server TEXT, date INTEGER NOT NULL, gold_income INTEGER DEFAULT 0, gold_expense INTEGER DEFAULT 0, notes TEXT, record_type TEXT DEFAULT 'baizhan', updated_at TEXT);
CREATE TABLE favorite_raids (id INTEGER PRIMARY KEY AUTOINCREMENT, raid_name TEXT NOT NULL UNIQUE, created_at TEXT NOT NULL);
${BASE_TABLES}
INSERT INTO accounts (id, account_name, account_type, created_at, updated_at) VALUES ('test-acc-1', '测试账号1', 'OWN', '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z');
INSERT INTO roles (id, account_id, name, server, sect, created_at, updated_at) VALUES ('role-1', 'test-acc-1', '测试角色1', '电信五区', '少林', '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z');
INSERT INTO favorite_raids (raid_name, created_at) VALUES ('达摩洞', '2024-01-04T00:00:00Z');
`;
  createDatabase(dbPath, sql);
  console.log(`V4 数据库创建成功: ${dbPath}`);
}

function createV5Database(dbPath) {
  const sql = `
CREATE TABLE schema_versions (version INTEGER PRIMARY KEY, applied_at TEXT NOT NULL, description TEXT);
INSERT INTO schema_versions VALUES (1, '2024-01-01T00:00:00Z', 'V1'), (2, '2024-01-02T00:00:00Z', 'V2'), (3, '2024-01-03T00:00:00Z', 'V3'), (4, '2024-01-04T00:00:00Z', 'V4'), (5, '2024-01-05T00:00:00Z', 'V5');
CREATE TABLE accounts (id TEXT PRIMARY KEY, account_name TEXT NOT NULL, account_type TEXT NOT NULL DEFAULT 'OWN', sort_order INTEGER DEFAULT 0, password TEXT, notes TEXT, hidden INTEGER DEFAULT 0, disabled INTEGER DEFAULT 0, created_at TEXT, updated_at TEXT);
CREATE TABLE roles (id TEXT PRIMARY KEY, account_id TEXT NOT NULL, name TEXT NOT NULL, server TEXT, region TEXT, sect TEXT, martial TEXT, equipment_score INTEGER, disabled INTEGER DEFAULT 0, created_at TEXT, updated_at TEXT);
CREATE TABLE raids (id TEXT PRIMARY KEY, name TEXT NOT NULL, difficulty TEXT NOT NULL DEFAULT '普通', player_count INTEGER NOT NULL DEFAULT 25, version TEXT, notes TEXT, is_active INTEGER DEFAULT 1, is_static INTEGER DEFAULT 0);
CREATE TABLE raid_bosses (id TEXT PRIMARY KEY, raid_name TEXT NOT NULL, name TEXT NOT NULL, boss_order INTEGER NOT NULL);
CREATE TABLE raid_versions (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT UNIQUE NOT NULL);
CREATE TABLE trial_records (id TEXT PRIMARY KEY, account_id TEXT, role_id TEXT, role_name TEXT DEFAULT '', server TEXT DEFAULT '', layer INTEGER, bosses TEXT, card_1 TEXT, card_2 TEXT, card_3 TEXT, card_4 TEXT, card_5 TEXT, flipped_index INTEGER, record_type TEXT DEFAULT 'trial', date INTEGER NOT NULL, notes TEXT, updated_at TEXT);
CREATE TABLE baizhan_records (id TEXT PRIMARY KEY, account_id TEXT NOT NULL, role_id TEXT NOT NULL, role_name TEXT, server TEXT, date INTEGER NOT NULL, gold_income INTEGER DEFAULT 0, gold_expense INTEGER DEFAULT 0, notes TEXT, record_type TEXT DEFAULT 'baizhan', updated_at TEXT);
CREATE TABLE favorite_raids (id INTEGER PRIMARY KEY AUTOINCREMENT, raid_name TEXT NOT NULL UNIQUE, created_at TEXT NOT NULL);
CREATE TABLE instance_types (id INTEGER PRIMARY KEY AUTOINCREMENT, type TEXT NOT NULL UNIQUE, name TEXT NOT NULL);
CREATE TABLE role_instance_visibility (id TEXT PRIMARY KEY, role_id TEXT NOT NULL, instance_type_id INTEGER NOT NULL, visible INTEGER DEFAULT 1, created_at TEXT, updated_at TEXT);
${BASE_TABLES}
INSERT INTO accounts (id, account_name, account_type, created_at, updated_at) VALUES ('test-acc-1', '测试账号1', 'OWN', '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z');
INSERT INTO roles (id, account_id, name, server, sect, created_at, updated_at) VALUES ('role-1', 'test-acc-1', '测试角色1', '电信五区', '少林', '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z');
INSERT INTO instance_types (type, name) VALUES ('raid', '团队副本'), ('baizhan', '百战'), ('trial', '试炼之地');
INSERT INTO role_instance_visibility (id, role_id, instance_type_id, visible, created_at, updated_at) VALUES ('riv-1', 'role-1', 1, 1, '2024-01-05T00:00:00Z', '2024-01-05T00:00:00Z');
`;
  createDatabase(dbPath, sql);
  console.log(`V5 数据库创建成功: ${dbPath}`);
}

function createV6Database(dbPath) {
  const sql = `
CREATE TABLE schema_versions (version INTEGER PRIMARY KEY, applied_at TEXT NOT NULL, description TEXT);
INSERT INTO schema_versions VALUES (1, '2024-01-01T00:00:00Z', 'V1'), (2, '2024-01-02T00:00:00Z', 'V2'), (3, '2024-01-03T00:00:00Z', 'V3'), (4, '2024-01-04T00:00:00Z', 'V4'), (5, '2024-01-05T00:00:00Z', 'V5'), (6, '2024-01-06T00:00:00Z', 'V6');
CREATE TABLE accounts (id TEXT PRIMARY KEY, account_name TEXT NOT NULL, account_type TEXT NOT NULL DEFAULT 'OWN', sort_order INTEGER DEFAULT 0, password TEXT, notes TEXT, hidden INTEGER DEFAULT 0, disabled INTEGER DEFAULT 0, created_at TEXT, updated_at TEXT);
CREATE TABLE roles (id TEXT PRIMARY KEY, account_id TEXT NOT NULL, name TEXT NOT NULL, server TEXT, region TEXT, sect TEXT, martial TEXT, equipment_score INTEGER, disabled INTEGER DEFAULT 0, created_at TEXT, updated_at TEXT);
CREATE TABLE raids (id TEXT PRIMARY KEY, name TEXT NOT NULL, difficulty TEXT NOT NULL DEFAULT '普通', player_count INTEGER NOT NULL DEFAULT 25, version TEXT, notes TEXT, is_active INTEGER DEFAULT 1, is_static INTEGER DEFAULT 0);
CREATE TABLE raid_bosses (id TEXT PRIMARY KEY, raid_name TEXT NOT NULL, name TEXT NOT NULL, boss_order INTEGER NOT NULL);
CREATE TABLE raid_versions (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT UNIQUE NOT NULL);
CREATE TABLE trial_records (id TEXT PRIMARY KEY, account_id TEXT, role_id TEXT, role_name TEXT DEFAULT '', server TEXT DEFAULT '', layer INTEGER, bosses TEXT, card_1 TEXT, card_2 TEXT, card_3 TEXT, card_4 TEXT, card_5 TEXT, flipped_index INTEGER, record_type TEXT DEFAULT 'trial', date INTEGER NOT NULL, notes TEXT, updated_at TEXT);
CREATE TABLE baizhan_records (id TEXT PRIMARY KEY, account_id TEXT NOT NULL, role_id TEXT NOT NULL, role_name TEXT, server TEXT, date INTEGER NOT NULL, gold_income INTEGER DEFAULT 0, gold_expense INTEGER DEFAULT 0, notes TEXT, record_type TEXT DEFAULT 'baizhan', updated_at TEXT);
CREATE TABLE favorite_raids (id INTEGER PRIMARY KEY AUTOINCREMENT, raid_name TEXT NOT NULL UNIQUE, created_at TEXT NOT NULL);
CREATE TABLE instance_types (id INTEGER PRIMARY KEY AUTOINCREMENT, type TEXT NOT NULL UNIQUE, name TEXT NOT NULL);
CREATE TABLE role_instance_visibility (id TEXT PRIMARY KEY, role_id TEXT NOT NULL, instance_type_id INTEGER NOT NULL, visible INTEGER DEFAULT 1, created_at TEXT, updated_at TEXT);
CREATE TABLE raid_role_visibility (id TEXT PRIMARY KEY, role_id TEXT NOT NULL, raid_key TEXT NOT NULL, visible INTEGER DEFAULT 1, created_at TEXT, updated_at TEXT);
${BASE_TABLES}
INSERT INTO accounts (id, account_name, account_type, created_at, updated_at) VALUES ('test-acc-1', '测试账号1', 'OWN', '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z');
INSERT INTO roles (id, account_id, name, server, sect, created_at, updated_at) VALUES ('role-1', 'test-acc-1', '测试角色1', '电信五区', '少林', '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z');
INSERT INTO instance_types (type, name) VALUES ('raid', '团队副本'), ('baizhan', '百战'), ('trial', '试炼之地');
`;
  createDatabase(dbPath, sql);
  console.log(`V6 数据库创建成功: ${dbPath}`);
}

function createV7Database(dbPath) {
  const sql = `
CREATE TABLE schema_versions (version INTEGER PRIMARY KEY, applied_at TEXT NOT NULL, description TEXT);
INSERT INTO schema_versions VALUES (1, '2024-01-01T00:00:00Z', 'V1'), (2, '2024-01-02T00:00:00Z', 'V2'), (3, '2024-01-03T00:00:00Z', 'V3'), (4, '2024-01-04T00:00:00Z', 'V4'), (5, '2024-01-05T00:00:00Z', 'V5'), (6, '2024-01-06T00:00:00Z', 'V6'), (7, '2024-01-07T00:00:00Z', 'V7');
CREATE TABLE accounts (id TEXT PRIMARY KEY, account_name TEXT NOT NULL, account_type TEXT NOT NULL DEFAULT 'OWN', sort_order INTEGER DEFAULT 0, password TEXT, notes TEXT, hidden INTEGER DEFAULT 0, disabled INTEGER DEFAULT 0, created_at TEXT, updated_at TEXT);
CREATE TABLE roles (id TEXT PRIMARY KEY, account_id TEXT NOT NULL, name TEXT NOT NULL, server TEXT, region TEXT, sect TEXT, martial TEXT, equipment_score INTEGER, disabled INTEGER DEFAULT 0, created_at TEXT, updated_at TEXT);
CREATE TABLE raids (id TEXT PRIMARY KEY, name TEXT NOT NULL, difficulty TEXT NOT NULL DEFAULT '普通', player_count INTEGER NOT NULL DEFAULT 25, version TEXT, notes TEXT, is_active INTEGER DEFAULT 1, is_static INTEGER DEFAULT 0);
CREATE TABLE raid_bosses (id TEXT PRIMARY KEY, raid_name TEXT NOT NULL, name TEXT NOT NULL, boss_order INTEGER NOT NULL);
CREATE TABLE raid_versions (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT UNIQUE NOT NULL);
CREATE TABLE trial_records (id TEXT PRIMARY KEY, account_id TEXT, role_id TEXT, role_name TEXT DEFAULT '', server TEXT DEFAULT '', layer INTEGER, bosses TEXT, card_1 TEXT, card_2 TEXT, card_3 TEXT, card_4 TEXT, card_5 TEXT, flipped_index INTEGER, record_type TEXT DEFAULT 'trial', date INTEGER NOT NULL, notes TEXT, updated_at TEXT);
CREATE TABLE baizhan_records (id TEXT PRIMARY KEY, account_id TEXT NOT NULL, role_id TEXT NOT NULL, role_name TEXT, server TEXT, date INTEGER NOT NULL, gold_income INTEGER DEFAULT 0, gold_expense INTEGER DEFAULT 0, notes TEXT, record_type TEXT DEFAULT 'baizhan', updated_at TEXT);
CREATE TABLE favorite_raids (id INTEGER PRIMARY KEY AUTOINCREMENT, raid_name TEXT NOT NULL UNIQUE, created_at TEXT NOT NULL);
CREATE TABLE instance_types (id INTEGER PRIMARY KEY AUTOINCREMENT, type TEXT NOT NULL UNIQUE, name TEXT NOT NULL);
CREATE TABLE role_instance_visibility (id TEXT PRIMARY KEY, role_id TEXT NOT NULL, instance_type_id INTEGER NOT NULL, visible INTEGER DEFAULT 1, created_at TEXT, updated_at TEXT);
CREATE TABLE raid_role_visibility (id TEXT PRIMARY KEY, role_id TEXT NOT NULL, raid_key TEXT NOT NULL, visible INTEGER DEFAULT 1, created_at TEXT, updated_at TEXT);
${BASE_TABLES}
INSERT INTO accounts (id, account_name, account_type, sort_order, created_at, updated_at) VALUES ('test-acc-1', '测试账号1', 'OWN', 0, '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z');
INSERT INTO roles (id, account_id, name, server, sect, created_at, updated_at) VALUES ('role-1', 'test-acc-1', '测试角色1', '电信五区', '少林', '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z');
INSERT INTO instance_types (type, name) VALUES ('raid', '团队副本'), ('baizhan', '百战'), ('trial', '试炼之地');
`;
  createDatabase(dbPath, sql);
  console.log(`V7 数据库创建成功: ${dbPath}`);
}

function createV8Database(dbPath) {
  const sql = `
CREATE TABLE schema_versions (version INTEGER PRIMARY KEY, applied_at TEXT NOT NULL, description TEXT);
INSERT INTO schema_versions VALUES (1, '2024-01-01T00:00:00Z', 'V1'), (2, '2024-01-02T00:00:00Z', 'V2'), (3, '2024-01-03T00:00:00Z', 'V3'), (4, '2024-01-04T00:00:00Z', 'V4'), (5, '2024-01-05T00:00:00Z', 'V5'), (6, '2024-01-06T00:00:00Z', 'V6'), (7, '2024-01-07T00:00:00Z', 'V7'), (8, '2024-01-08T00:00:00Z', 'V8');
CREATE TABLE accounts (id TEXT PRIMARY KEY, account_name TEXT NOT NULL, account_type TEXT NOT NULL DEFAULT 'OWN', sort_order INTEGER DEFAULT 0, password TEXT, notes TEXT, hidden INTEGER DEFAULT 0, disabled INTEGER DEFAULT 0, created_at TEXT, updated_at TEXT);
CREATE TABLE roles (id TEXT PRIMARY KEY, account_id TEXT NOT NULL, name TEXT NOT NULL, server TEXT, region TEXT, sect TEXT, martial TEXT, equipment_score INTEGER, disabled INTEGER DEFAULT 0, created_at TEXT, updated_at TEXT);
CREATE TABLE raids (id TEXT PRIMARY KEY, name TEXT NOT NULL, difficulty TEXT NOT NULL DEFAULT '普通', player_count INTEGER NOT NULL DEFAULT 25, version TEXT, notes TEXT, is_active INTEGER DEFAULT 1, is_static INTEGER DEFAULT 0);
CREATE TABLE raid_bosses (id TEXT PRIMARY KEY, raid_name TEXT NOT NULL, name TEXT NOT NULL, boss_order INTEGER NOT NULL);
CREATE TABLE raid_versions (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT UNIQUE NOT NULL);
CREATE TABLE trial_records (id TEXT PRIMARY KEY, account_id TEXT, role_id TEXT, role_name TEXT DEFAULT '', server TEXT DEFAULT '', layer INTEGER, bosses TEXT, card_1 TEXT, card_2 TEXT, card_3 TEXT, card_4 TEXT, card_5 TEXT, flipped_index INTEGER, record_type TEXT DEFAULT 'trial', date INTEGER NOT NULL, notes TEXT, updated_at TEXT);
CREATE TABLE baizhan_records (id TEXT PRIMARY KEY, account_id TEXT NOT NULL, role_id TEXT NOT NULL, role_name TEXT, server TEXT, date INTEGER NOT NULL, gold_income INTEGER DEFAULT 0, gold_expense INTEGER DEFAULT 0, notes TEXT, record_type TEXT DEFAULT 'baizhan', updated_at TEXT);
CREATE TABLE favorite_raids (id INTEGER PRIMARY KEY AUTOINCREMENT, raid_name TEXT NOT NULL UNIQUE, created_at TEXT NOT NULL);
CREATE TABLE instance_types (id INTEGER PRIMARY KEY AUTOINCREMENT, type TEXT NOT NULL UNIQUE, name TEXT NOT NULL);
CREATE TABLE role_instance_visibility (id TEXT PRIMARY KEY, role_id TEXT NOT NULL, instance_type_id INTEGER NOT NULL, visible INTEGER DEFAULT 1, created_at TEXT, updated_at TEXT);
CREATE TABLE raid_role_visibility (id TEXT PRIMARY KEY, role_id TEXT NOT NULL, raid_key TEXT NOT NULL, visible INTEGER DEFAULT 1, created_at TEXT, updated_at TEXT);
${BASE_TABLES}
INSERT INTO accounts (id, account_name, account_type, sort_order, created_at, updated_at) VALUES ('test-acc-1', '测试账号1', 'OWN', 0, '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z');
INSERT INTO roles (id, account_id, name, server, sect, martial, created_at, updated_at) VALUES ('role-1', 'test-acc-1', '测试角色1', '电信五区', '少林', '易筋经', '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z');
INSERT INTO instance_types (type, name) VALUES ('raid', '团队副本'), ('baizhan', '百战'), ('trial', '试炼之地');
`;
  createDatabase(dbPath, sql);
  console.log(`V8 数据库创建成功: ${dbPath}`);
}

function createV9Database(dbPath) {
  const sql = `
CREATE TABLE schema_versions (version INTEGER PRIMARY KEY, applied_at TEXT NOT NULL, description TEXT);
INSERT INTO schema_versions VALUES (1, '2024-01-01T00:00:00Z', 'V1'), (2, '2024-01-02T00:00:00Z', 'V2'), (3, '2024-01-03T00:00:00Z', 'V3'), (4, '2024-01-04T00:00:00Z', 'V4'), (5, '2024-01-05T00:00:00Z', 'V5'), (6, '2024-01-06T00:00:00Z', 'V6'), (7, '2024-01-07T00:00:00Z', 'V7'), (8, '2024-01-08T00:00:00Z', 'V8'), (9, '2024-01-09T00:00:00Z', 'V9');
CREATE TABLE accounts (id TEXT PRIMARY KEY, account_name TEXT NOT NULL, account_type TEXT NOT NULL DEFAULT 'OWN', sort_order INTEGER DEFAULT 0, password TEXT, notes TEXT, hidden INTEGER DEFAULT 0, disabled INTEGER DEFAULT 0, created_at TEXT, updated_at TEXT);
CREATE TABLE roles (id TEXT PRIMARY KEY, account_id TEXT NOT NULL, name TEXT NOT NULL, server TEXT, region TEXT, sect TEXT, martial TEXT, equipment_score INTEGER, disabled INTEGER DEFAULT 0, created_at TEXT, updated_at TEXT);
CREATE TABLE raids (id TEXT PRIMARY KEY, name TEXT NOT NULL, difficulty TEXT NOT NULL DEFAULT '普通', player_count INTEGER NOT NULL DEFAULT 25, version TEXT, notes TEXT, is_active INTEGER DEFAULT 1, is_static INTEGER DEFAULT 0);
CREATE TABLE raid_bosses (id TEXT PRIMARY KEY, raid_name TEXT NOT NULL, name TEXT NOT NULL, boss_order INTEGER NOT NULL);
CREATE TABLE raid_versions (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT UNIQUE NOT NULL);
CREATE TABLE trial_records (id TEXT PRIMARY KEY, account_id TEXT, role_id TEXT, role_name TEXT DEFAULT '', server TEXT DEFAULT '', layer INTEGER, bosses TEXT, card_1 TEXT, card_2 TEXT, card_3 TEXT, card_4 TEXT, card_5 TEXT, flipped_index INTEGER, record_type TEXT DEFAULT 'trial', date INTEGER NOT NULL, notes TEXT, updated_at TEXT);
CREATE TABLE baizhan_records (id TEXT PRIMARY KEY, account_id TEXT NOT NULL, role_id TEXT NOT NULL, role_name TEXT, server TEXT, date INTEGER NOT NULL, gold_income INTEGER DEFAULT 0, gold_expense INTEGER DEFAULT 0, notes TEXT, record_type TEXT DEFAULT 'baizhan', updated_at TEXT);
CREATE TABLE favorite_raids (id INTEGER PRIMARY KEY AUTOINCREMENT, raid_name TEXT NOT NULL UNIQUE, created_at TEXT NOT NULL);
CREATE TABLE instance_types (id INTEGER PRIMARY KEY AUTOINCREMENT, type TEXT NOT NULL UNIQUE, name TEXT NOT NULL);
CREATE TABLE role_instance_visibility (id TEXT PRIMARY KEY, role_id TEXT NOT NULL, instance_type_id INTEGER NOT NULL, visible INTEGER DEFAULT 1, created_at TEXT, updated_at TEXT);
CREATE TABLE raid_role_visibility (id TEXT PRIMARY KEY, role_id TEXT NOT NULL, raid_key TEXT NOT NULL, visible INTEGER DEFAULT 1, created_at TEXT, updated_at TEXT);
${BASE_TABLES}
INSERT INTO accounts (id, account_name, account_type, sort_order, created_at, updated_at) VALUES ('test-acc-1', '测试账号1', 'OWN', 0, '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z');
INSERT INTO roles (id, account_id, name, server, sect, martial, created_at, updated_at) VALUES ('role-1', 'test-acc-1', '测试角色1', '电信五区', '少林', '易筋经', '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z');
INSERT INTO instance_types (type, name) VALUES ('raid', '团队副本'), ('baizhan', '百战'), ('trial', '试炼之地');
INSERT INTO raids (id, name, difficulty, player_count, version, is_static) VALUES ('25人普通阆风悬城', '阆风悬城', '普通', 25, '万灵当歌', 1);
`;
  createDatabase(dbPath, sql);
  console.log(`V9 数据库创建成功: ${dbPath}`);
}

function createV10Database(dbPath) {
  const sql = `
CREATE TABLE schema_versions (version INTEGER PRIMARY KEY, applied_at TEXT NOT NULL, description TEXT);
INSERT INTO schema_versions VALUES (1, '2024-01-01T00:00:00Z', 'V1'), (2, '2024-01-02T00:00:00Z', 'V2'), (3, '2024-01-03T00:00:00Z', 'V3'), (4, '2024-01-04T00:00:00Z', 'V4'), (5, '2024-01-05T00:00:00Z', 'V5'), (6, '2024-01-06T00:00:00Z', 'V6'), (7, '2024-01-07T00:00:00Z', 'V7'), (8, '2024-01-08T00:00:00Z', 'V8'), (9, '2024-01-09T00:00:00Z', 'V9'), (10, '2024-01-10T00:00:00Z', 'V10');
CREATE TABLE accounts (id TEXT PRIMARY KEY, account_name TEXT NOT NULL, account_type TEXT NOT NULL DEFAULT 'OWN', sort_order INTEGER DEFAULT 0, password TEXT, notes TEXT, hidden INTEGER DEFAULT 0, disabled INTEGER DEFAULT 0, created_at TEXT, updated_at TEXT);
CREATE TABLE roles (id TEXT PRIMARY KEY, account_id TEXT NOT NULL, name TEXT NOT NULL, server TEXT, region TEXT, sect TEXT, martial TEXT, equipment_score INTEGER, disabled INTEGER DEFAULT 0, created_at TEXT, updated_at TEXT);
CREATE TABLE raids (id TEXT PRIMARY KEY, name TEXT NOT NULL, difficulty TEXT NOT NULL DEFAULT '普通', player_count INTEGER NOT NULL DEFAULT 25, version TEXT, notes TEXT, is_active INTEGER DEFAULT 1, is_static INTEGER DEFAULT 0, season_id INTEGER);
CREATE TABLE raid_bosses (id TEXT PRIMARY KEY, raid_name TEXT NOT NULL, name TEXT NOT NULL, boss_order INTEGER NOT NULL);
CREATE TABLE raid_versions (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT UNIQUE NOT NULL);
CREATE TABLE trial_records (id TEXT PRIMARY KEY, account_id TEXT, role_id TEXT, role_name TEXT DEFAULT '', server TEXT DEFAULT '', layer INTEGER, bosses TEXT, card_1 TEXT, card_2 TEXT, card_3 TEXT, card_4 TEXT, card_5 TEXT, flipped_index INTEGER, record_type TEXT DEFAULT 'trial', date INTEGER NOT NULL, notes TEXT, updated_at TEXT);
CREATE TABLE baizhan_records (id TEXT PRIMARY KEY, account_id TEXT NOT NULL, role_id TEXT NOT NULL, role_name TEXT, server TEXT, date INTEGER NOT NULL, gold_income INTEGER DEFAULT 0, gold_expense INTEGER DEFAULT 0, notes TEXT, record_type TEXT DEFAULT 'baizhan', updated_at TEXT);
CREATE TABLE favorite_raids (id INTEGER PRIMARY KEY AUTOINCREMENT, raid_name TEXT NOT NULL UNIQUE, created_at TEXT NOT NULL);
CREATE TABLE instance_types (id INTEGER PRIMARY KEY AUTOINCREMENT, type TEXT NOT NULL UNIQUE, name TEXT NOT NULL);
CREATE TABLE role_instance_visibility (id TEXT PRIMARY KEY, role_id TEXT NOT NULL, instance_type_id INTEGER NOT NULL, visible INTEGER DEFAULT 1, created_at TEXT, updated_at TEXT);
CREATE TABLE raid_role_visibility (id TEXT PRIMARY KEY, role_id TEXT NOT NULL, raid_key TEXT NOT NULL, visible INTEGER DEFAULT 1, created_at TEXT, updated_at TEXT);
CREATE TABLE game_versions (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL UNIQUE, sort_order INTEGER NOT NULL DEFAULT 0, created_at TEXT NOT NULL);
CREATE TABLE seasons (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL, version_id INTEGER NOT NULL, start_date INTEGER NOT NULL, end_date INTEGER, sort_order INTEGER NOT NULL DEFAULT 0, trial_equip_level_min INTEGER DEFAULT 0, trial_equip_level_max INTEGER DEFAULT 0, created_at TEXT NOT NULL);
${BASE_TABLES}
INSERT INTO accounts (id, account_name, account_type, sort_order, created_at, updated_at) VALUES ('test-acc-1', '测试账号1', 'OWN', 0, '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z');
INSERT INTO roles (id, account_id, name, server, sect, martial, created_at, updated_at) VALUES ('role-1', 'test-acc-1', '测试角色1', '电信五区', '少林', '易筋经', '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z');
INSERT INTO instance_types (type, name) VALUES ('raid', '团队副本'), ('baizhan', '百战'), ('trial', '试炼之地');
INSERT INTO game_versions (name, sort_order, created_at) VALUES ('万灵当歌', 1, '2024-01-01T00:00:00Z');
INSERT INTO seasons (name, version_id, start_date, end_date, sort_order, created_at) VALUES ('S1', 1, 1704067200, 1735689600, 1, '2024-01-01T00:00:00Z');
`;
  createDatabase(dbPath, sql);
  console.log(`V10 数据库创建成功: ${dbPath}`);
}

function createV11Database(dbPath) {
  const sql = `
CREATE TABLE schema_versions (version INTEGER PRIMARY KEY, applied_at TEXT NOT NULL, description TEXT);
INSERT INTO schema_versions VALUES (1, '2024-01-01T00:00:00Z', 'V1'), (2, '2024-01-02T00:00:00Z', 'V2'), (3, '2024-01-03T00:00:00Z', 'V3'), (4, '2024-01-04T00:00:00Z', 'V4'), (5, '2024-01-05T00:00:00Z', 'V5'), (6, '2024-01-06T00:00:00Z', 'V6'), (7, '2024-01-07T00:00:00Z', 'V7'), (8, '2024-01-08T00:00:00Z', 'V8'), (9, '2024-01-09T00:00:00Z', 'V9'), (10, '2024-01-10T00:00:00Z', 'V10'), (11, '2024-01-11T00:00:00Z', 'V11');
CREATE TABLE accounts (id TEXT PRIMARY KEY, account_name TEXT NOT NULL, account_type TEXT NOT NULL DEFAULT 'OWN', sort_order INTEGER DEFAULT 0, password TEXT, notes TEXT, hidden INTEGER DEFAULT 0, disabled INTEGER DEFAULT 0, created_at TEXT, updated_at TEXT);
CREATE TABLE roles (id TEXT PRIMARY KEY, account_id TEXT NOT NULL, name TEXT NOT NULL, server TEXT, region TEXT, sect TEXT, martial TEXT, equipment_score INTEGER, disabled INTEGER DEFAULT 0, created_at TEXT, updated_at TEXT);
CREATE TABLE raids (id TEXT PRIMARY KEY, name TEXT NOT NULL, difficulty TEXT NOT NULL DEFAULT '普通', player_count INTEGER NOT NULL DEFAULT 25, version TEXT, notes TEXT, is_active INTEGER DEFAULT 1, is_static INTEGER DEFAULT 0, season_id INTEGER);
CREATE TABLE raid_bosses (id TEXT PRIMARY KEY, raid_name TEXT NOT NULL, name TEXT NOT NULL, boss_order INTEGER NOT NULL);
CREATE TABLE raid_versions (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT UNIQUE NOT NULL);
CREATE TABLE trial_records (id TEXT PRIMARY KEY, account_id TEXT, role_id TEXT, role_name TEXT DEFAULT '', server TEXT DEFAULT '', layer INTEGER, bosses TEXT, card_1 TEXT, card_2 TEXT, card_3 TEXT, card_4 TEXT, card_5 TEXT, flipped_index INTEGER, record_type TEXT DEFAULT 'trial', date INTEGER NOT NULL, notes TEXT, updated_at TEXT);
CREATE TABLE baizhan_records (id TEXT PRIMARY KEY, account_id TEXT NOT NULL, role_id TEXT NOT NULL, role_name TEXT, server TEXT, date INTEGER NOT NULL, gold_income INTEGER DEFAULT 0, gold_expense INTEGER DEFAULT 0, notes TEXT, record_type TEXT DEFAULT 'baizhan', updated_at TEXT);
CREATE TABLE favorite_raids (id INTEGER PRIMARY KEY AUTOINCREMENT, raid_name TEXT NOT NULL UNIQUE, created_at TEXT NOT NULL);
CREATE TABLE instance_types (id INTEGER PRIMARY KEY AUTOINCREMENT, type TEXT NOT NULL UNIQUE, name TEXT NOT NULL);
CREATE TABLE role_instance_visibility (id TEXT PRIMARY KEY, role_id TEXT NOT NULL, instance_type_id INTEGER NOT NULL, visible INTEGER DEFAULT 1, created_at TEXT, updated_at TEXT);
CREATE TABLE raid_role_visibility (id TEXT PRIMARY KEY, role_id TEXT NOT NULL, raid_key TEXT NOT NULL, visible INTEGER DEFAULT 1, created_at TEXT, updated_at TEXT);
CREATE TABLE game_versions (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL UNIQUE, sort_order INTEGER NOT NULL DEFAULT 0, created_at TEXT NOT NULL);
CREATE TABLE seasons (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL, version_id INTEGER NOT NULL, start_date INTEGER NOT NULL, end_date INTEGER, sort_order INTEGER NOT NULL DEFAULT 0, trial_equip_level_min INTEGER DEFAULT 0, trial_equip_level_max INTEGER DEFAULT 0, created_at TEXT NOT NULL);
CREATE TABLE equipments (id TEXT PRIMARY KEY, name TEXT NOT NULL, icon_id INTEGER, quality INTEGER, updated_at TEXT);
${BASE_TABLES}
INSERT INTO accounts (id, account_name, account_type, sort_order, created_at, updated_at) VALUES ('test-acc-1', '测试账号1', 'OWN', 0, '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z');
INSERT INTO roles (id, account_id, name, server, sect, martial, created_at, updated_at) VALUES ('role-1', 'test-acc-1', '测试角色1', '电信五区', '少林', '易筋经', '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z');
INSERT INTO instance_types (type, name) VALUES ('raid', '团队副本'), ('baizhan', '百战'), ('trial', '试炼之地');
INSERT INTO equipments (id, name, icon_id, quality, updated_at) VALUES ('12345', '测试装备1', 100, 5, '2024-01-01T00:00:00Z'), ('12345_67890', '测试装备1', 100, 5, '2024-01-01T00:00:00Z');
`;
  createDatabase(dbPath, sql);
  console.log(`V11 数据库创建成功: ${dbPath}`);
}

function createV12Database(dbPath) {
  const sql = `
CREATE TABLE schema_versions (version INTEGER PRIMARY KEY, applied_at TEXT NOT NULL, description TEXT);
INSERT INTO schema_versions VALUES (1, '2024-01-01T00:00:00Z', 'V1'), (2, '2024-01-02T00:00:00Z', 'V2'), (3, '2024-01-03T00:00:00Z', 'V3'), (4, '2024-01-04T00:00:00Z', 'V4'), (5, '2024-01-05T00:00:00Z', 'V5'), (6, '2024-01-06T00:00:00Z', 'V6'), (7, '2024-01-07T00:00:00Z', 'V7'), (8, '2024-01-08T00:00:00Z', 'V8'), (9, '2024-01-09T00:00:00Z', 'V9'), (10, '2024-01-10T00:00:00Z', 'V10'), (11, '2024-01-11T00:00:00Z', 'V11'), (12, '2024-01-12T00:00:00Z', 'V12');
CREATE TABLE accounts (id TEXT PRIMARY KEY, account_name TEXT NOT NULL, account_type TEXT NOT NULL DEFAULT 'OWN', sort_order INTEGER DEFAULT 0, password TEXT, notes TEXT, hidden INTEGER DEFAULT 0, disabled INTEGER DEFAULT 0, created_at TEXT, updated_at TEXT);
CREATE TABLE roles (id TEXT PRIMARY KEY, account_id TEXT NOT NULL, name TEXT NOT NULL, server TEXT, region TEXT, sect TEXT, martial TEXT, equipment_score INTEGER, disabled INTEGER DEFAULT 0, created_at TEXT, updated_at TEXT);
CREATE TABLE raids (id TEXT PRIMARY KEY, name TEXT NOT NULL, difficulty TEXT NOT NULL DEFAULT '普通', player_count INTEGER NOT NULL DEFAULT 25, version TEXT, notes TEXT, is_active INTEGER DEFAULT 1, is_static INTEGER DEFAULT 0, season_id INTEGER);
CREATE TABLE raid_bosses (id TEXT PRIMARY KEY, raid_name TEXT NOT NULL, name TEXT NOT NULL, boss_order INTEGER NOT NULL);
CREATE TABLE raid_versions (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT UNIQUE NOT NULL);
CREATE TABLE trial_records (id TEXT PRIMARY KEY, account_id TEXT, role_id TEXT, role_name TEXT DEFAULT '', server TEXT DEFAULT '', layer INTEGER, bosses TEXT, card_1 TEXT, card_2 TEXT, card_3 TEXT, card_4 TEXT, card_5 TEXT, flipped_index INTEGER, record_type TEXT DEFAULT 'trial', date INTEGER NOT NULL, notes TEXT, updated_at TEXT);
CREATE TABLE baizhan_records (id TEXT PRIMARY KEY, account_id TEXT NOT NULL, role_id TEXT NOT NULL, role_name TEXT, server TEXT, date INTEGER NOT NULL, gold_income INTEGER DEFAULT 0, gold_expense INTEGER DEFAULT 0, notes TEXT, record_type TEXT DEFAULT 'baizhan', updated_at TEXT);
CREATE TABLE favorite_raids (id INTEGER PRIMARY KEY AUTOINCREMENT, raid_name TEXT NOT NULL UNIQUE, created_at TEXT NOT NULL);
CREATE TABLE instance_types (id INTEGER PRIMARY KEY AUTOINCREMENT, type TEXT NOT NULL UNIQUE, name TEXT NOT NULL);
CREATE TABLE role_instance_visibility (id TEXT PRIMARY KEY, role_id TEXT NOT NULL, instance_type_id INTEGER NOT NULL, visible INTEGER DEFAULT 1, created_at TEXT, updated_at TEXT);
CREATE TABLE raid_role_visibility (id TEXT PRIMARY KEY, role_id TEXT NOT NULL, raid_key TEXT NOT NULL, visible INTEGER DEFAULT 1, created_at TEXT, updated_at TEXT);
CREATE TABLE game_versions (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL UNIQUE, sort_order INTEGER NOT NULL DEFAULT 0, created_at TEXT NOT NULL);
CREATE TABLE seasons (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL, version_id INTEGER NOT NULL, start_date INTEGER NOT NULL, end_date INTEGER, sort_order INTEGER NOT NULL DEFAULT 0, trial_equip_level_min INTEGER DEFAULT 0, trial_equip_level_max INTEGER DEFAULT 0, created_at TEXT NOT NULL);
CREATE TABLE equipments (id TEXT PRIMARY KEY, name TEXT NOT NULL, icon_id INTEGER, quality INTEGER, updated_at TEXT);
CREATE TABLE records (id TEXT PRIMARY KEY, data TEXT, raid_name TEXT, account_id TEXT, role_id TEXT, record_date INTEGER, record_type TEXT);
${BASE_TABLES}
INSERT INTO accounts (id, account_name, account_type, sort_order, created_at, updated_at) VALUES ('test-acc-1', '测试账号1', 'OWN', 0, '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z');
INSERT INTO roles (id, account_id, name, server, sect, martial, created_at, updated_at) VALUES ('role-1', 'test-acc-1', '测试角色1', '电信五区', '少林', '易筋经', '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z');
INSERT INTO instance_types (type, name) VALUES ('raid', '团队副本'), ('baizhan', '百战'), ('trial', '试炼之地');
INSERT INTO records (id, data, raid_name, account_id, role_id, record_date, record_type) VALUES ('record-1', '{"id":"record-1","raidName":"测试副本"}', '测试副本', 'test-acc-1', 'role-1', 1704067200000, 'raid');
`;
  createDatabase(dbPath, sql);
  console.log(`V12 数据库创建成功: ${dbPath}`);
}

function verifyDatabase(dbPath, expectedVersion) {
  try {
    const result = execSync(`sqlite3 "${dbPath}" "SELECT version FROM schema_versions ORDER BY version DESC LIMIT 1"`, { encoding: 'utf8' }).trim();
    const actualVersion = parseInt(result, 10);
    if (actualVersion === expectedVersion) {
      console.log(`  ✓ 版本验证通过: V${actualVersion}`);
      return true;
    } else {
      console.log(`  ✗ 版本验证失败: 期望 V${expectedVersion}, 实际 V${actualVersion}`);
      return false;
    }
  } catch (error) {
    console.log(`  ✗ 数据库验证失败: ${error.message}`);
    return false;
  }
}

function main() {
  console.log('========================================');
  console.log('创建所有版本测试数据库');
  console.log('========================================\n');

  ensureDir(TestDir);
  console.log(`测试目录: ${TestDir}\n`);

  const creators = [
    { version: 1, create: createV1Database },
    { version: 2, create: createV2Database },
    { version: 3, create: createV3Database },
    { version: 4, create: createV4Database },
    { version: 5, create: createV5Database },
    { version: 6, create: createV6Database },
    { version: 7, create: createV7Database },
    { version: 8, create: createV8Database },
    { version: 9, create: createV9Database },
    { version: 10, create: createV10Database },
    { version: 11, create: createV11Database },
    { version: 12, create: createV12Database },
  ];

  for (const { version, create } of creators) {
    const dbPath = path.join(TestDir, `v${version}-test.db`);
    create(dbPath);
    verifyDatabase(dbPath, version);
  }

  console.log('\n========================================');
  console.log('所有测试数据库创建完成');
  console.log('========================================');
}

main();
