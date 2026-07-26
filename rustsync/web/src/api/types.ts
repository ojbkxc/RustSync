// API 通用响应
export interface ApiResponse<T> {
  code: number
  data: T
  msg?: string
}

// 分页参数
export interface PageParams {
  pageNum?: number
  pageSize?: number
}

// 分页响应
export interface PageData<T> {
  dataList: T[]
  count: number
}

// 用户
export interface LoginRequest {
  userName: string
  passwd: string
}

export interface ResetPasswordRequest {
  userName: string
  passwd?: string
  key: string
}

export interface ChangePasswordRequest {
  oldPasswd: string
  passwd: string
}

export interface UserInfo {
  id: number
  userName: string
  createTime: number
}

export interface LoginResponse extends UserInfo {
  token: string
}

// 引擎
export interface Engine {
  id: number
  remark?: string
  url: string
  userName?: string
  engineType: string
  systemKey?: string
  protected: boolean
  createTime: number
  displayName?: string
  directoryCount?: number
}

export interface EngineRequest {
  url: string
  token?: string
  remark?: string
  userName?: string
  engineType: string
}

// 存储挂载
export interface StorageMount {
  id: number
  engineId: number
  name: string
  driverType: string
  config: Record<string, unknown>
  enabled: boolean
  configVersion: number
  authVersion: number
  createTime: number
}

// 作业
export interface Job {
  id: number
  enable: boolean
  remark?: string
  srcPath: string
  dstPath: string
  alistId?: number
  useCacheT: boolean
  scanIntervalT: number
  useCacheS: boolean
  scanIntervalS: number
  method: number
  sourceMode: boolean
  interval?: number
  isCron: number
  year?: string
  month?: string
  day?: string
  week?: string
  dayOfWeek?: string
  hour?: string
  minute?: string
  second?: string
  startDate?: string
  endDate?: string
  exclude?: string
  minFileSize?: number
  maxFileSize?: number
  createTime: number
}

// 任务
export interface TaskInfo {
  id: number
  jobId: number
  status: number
  errMsg?: string
  runTime?: number
  createTime: number
  scanFinish: boolean
  doingTask: TaskItem[]
  duration: number
  firstSync?: number | null
  num: TaskNumStats
  size: TaskSizeStats
}

export interface TaskNumStats {
  waitNum: number
  runningNum: number
  successNum: number
  failNum: number
  otherNum: number
  allNum: number
}

export interface TaskSizeStats {
  wait: number
  running: number
  success: number
  fail: number
  other: number
}

export interface TaskItem {
  srcPath?: string
  dstPath?: string
  fileName?: string
  fileSize?: number
  type: number
  status: number
  progress?: number
  errMsg?: string
  createTime: number
}

// 通知
export interface Notify {
  id: number
  enable: boolean
  method: number
  params: string
  createTime: number
}

// 文件
export interface FileEntry {
  name: string
  path: string
  isDir: boolean
  size: number
  modified: number
  permissions: string
  extension: string
}

// 语言
export interface LanguageInfo {
  language: string
  languages: string[]
}

// 日志
export interface LogInfo {
  name: string
  size: number
  modified: number
}