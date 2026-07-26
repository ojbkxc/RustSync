import configparser
import logging
import os

from common.commonUtils import generatePasswd, readOrSet

sysConfig = None
DEFAULT_PASSWORD = 'admin'


def getPasswordStr():
    """
    获取加密字符串
    :return: 加密字符串
    """
    fileName = 'data/secret.key'
    passwdStr = readOrSet(fileName, generatePasswd(256))
    return passwdStr


def getConfig():
    global sysConfig
    if sysConfig is None:
        passwdStr = getPasswordStr()
        dbname = 'data/rustsync.db'
        sCfg = {
            'port': 8023,
            'expires': 2,
            'log_level': 1,
            'console_level': 2,
            'log_save': 7,
            'task_save': 0,
            'timeout': 72
        }
        password = DEFAULT_PASSWORD
        if os.path.exists('data/config.ini'):
            try:
                cfg = configparser.ConfigParser()
                cfg.read('data/config.ini', encoding='utf8')
                rustsync = cfg['rustsync']
                if 'password' in rustsync:
                    password = rustsync.get('password', raw=True)
                for keyItem in sCfg.keys():
                    if keyItem in rustsync:
                        sCfg[keyItem] = int(rustsync[keyItem])
            except Exception as e:
                logger = logging.getLogger()
                error_type = type(e).__name__
                logger.error(f"配置文件读取失败，将使用默认配置_/_config.ini read error: {error_type}")
        else:
            password = os.getenv('RUSTSYNC_PASSWORD')
            if password is None:
                password = os.getenv('RUSTSYNC_PASSWD', DEFAULT_PASSWORD)
            try:
                sCfg['port'] = int(os.getenv('RUSTSYNC_PORT', 8023))
                sCfg['expires'] = int(os.getenv('RUSTSYNC_EXPIRES', 2))
                sCfg['log_level'] = int(os.getenv('RUSTSYNC_LOG_LEVEL', 1))
                sCfg['console_level'] = int(os.getenv('RUSTSYNC_CONSOLE_LEVEL', 2))
                sCfg['log_save'] = int(os.getenv('RUSTSYNC_LOG_SAVE', 7))
                sCfg['task_save'] = int(os.getenv('RUSTSYNC_TASK_SAVE', 0))
                sCfg['timeout'] = int(os.getenv('RUSTSYNC_TASK_TIMEOUT', 72))
            except Exception as e:
                logger = logging.getLogger()
                logger.error(f"环境变量读取失败，将使用默认配置_/_ENV read error: {e}")
                logger.exception(e)
        if not password or not password.strip():
            password = DEFAULT_PASSWORD
        sysConfig = {
            'db': {
                'dbname': dbname
            },
            'server': {
                'passwdStr': passwdStr,
                'password': password,
                **sCfg
            }
        }
    return sysConfig
