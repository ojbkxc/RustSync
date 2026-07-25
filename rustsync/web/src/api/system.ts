import request from '@/utils/request'
import type { ApiResponse, LanguageInfo, LogInfo } from './types'

// GET /api/system/language
export function getLanguage() {
  return request<ApiResponse<LanguageInfo>>({ url: '/api/system/language', method: 'get' })
}

// POST /api/system/language
export function setLanguage(language: string) {
  return request<ApiResponse<null>>({ url: '/api/system/language', method: 'post', data: { language } })
}

// GET /api/system/logs
export function listLogs() {
  return request<ApiResponse<LogInfo[]>>({ url: '/api/system/logs', method: 'get' })
}

// GET /api/system/logs/read
export function readLogs(lines?: number) {
  return request<ApiResponse<{ lines: number; content: string }>>({
    url: '/api/system/logs/read',
    method: 'get',
    params: lines ? { lines } : undefined,
  })
}

// POST /api/system/logs/clear
export function clearLogs() {
  return request<ApiResponse<null>>({ url: '/api/system/logs/clear', method: 'post' })
}