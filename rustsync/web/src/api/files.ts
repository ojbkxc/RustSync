import request from '@/utils/request'
import type { ApiResponse, FileEntry } from './types'

// GET /api/files/list
export function listFiles(path: string) {
  return request<ApiResponse<FileEntry[]>>({ url: '/api/files/list', method: 'get', params: { path } })
}

// GET /api/files/read
export function readFile(path: string) {
  return request<ApiResponse<string>>({ url: '/api/files/read', method: 'get', params: { path } })
}

// POST /api/files/write
export function writeFile(path: string, content: string) {
  return request<ApiResponse<string>>({ url: '/api/files/write', method: 'post', data: { path, content } })
}

// POST /api/files/mkdir
export function createDir(path: string) {
  return request<ApiResponse<string>>({ url: '/api/files/mkdir', method: 'post', data: { path } })
}

// POST /api/files/touch
export function createFile(path: string) {
  return request<ApiResponse<string>>({ url: '/api/files/touch', method: 'post', data: { path } })
}

// POST /api/files/delete
export function deleteFile(path: string) {
  return request<ApiResponse<string>>({ url: '/api/files/delete', method: 'post', data: { path } })
}

// POST /api/files/rename
export function renameFile(from: string, to: string) {
  return request<ApiResponse<string>>({ url: '/api/files/rename', method: 'post', data: { from, to } })
}

// POST /api/files/copy
export function copyFile(from: string, to: string) {
  return request<ApiResponse<string>>({ url: '/api/files/copy', method: 'post', data: { from, to } })
}

// GET /api/files/info
export function fileInfo(path: string) {
  return request<ApiResponse<Record<string, unknown>>>({ url: '/api/files/info', method: 'get', params: { path } })
}

// POST /api/files/upload (multipart)
export function uploadFile(formData: FormData) {
  return request<ApiResponse<string>>({
    url: '/api/files/upload',
    method: 'post',
    data: formData,
    headers: { 'Content-Type': 'multipart/form-data' },
  })
}

// GET /api/files/download
export function downloadFile(path: string) {
  return request<Blob>({
    url: '/api/files/download',
    method: 'get',
    params: { path },
    responseType: 'blob',
  })
}

// GET /api/files/dirsize
export function dirSize(path: string) {
  return request<ApiResponse<{ path: string; size: number }>>({ url: '/api/files/dirsize', method: 'get', params: { path } })
}