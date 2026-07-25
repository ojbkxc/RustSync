import request from '@/utils/request'
import type { ApiResponse, Notify } from './types'

// GET /api/notifications
export function listNotifies() {
  return request<ApiResponse<Notify[]>>({ url: '/api/notifications', method: 'get' })
}

// POST /api/notifications
export function addNotify(notify: Record<string, unknown>) {
  return request<ApiResponse<null>>({ url: '/api/notifications', method: 'post', data: { notify } })
}

// PUT /api/notifications/:id
export function updateNotify(id: number, notify: Record<string, unknown>) {
  return request<ApiResponse<null>>({ url: `/api/notifications/${id}`, method: 'put', data: { notify } })
}

// PUT /api/notifications/:id/toggle
export function toggleNotify(id: number, enable: boolean) {
  return request<ApiResponse<null>>({ url: `/api/notifications/${id}/toggle`, method: 'put', data: { enable } })
}

// DELETE /api/notifications/:id
export function deleteNotify(id: number) {
  return request<ApiResponse<null>>({ url: `/api/notifications/${id}`, method: 'delete' })
}

// POST /api/notifications/test
export function testNotify(notify: Record<string, unknown>) {
  return request<ApiResponse<null>>({ url: '/api/notifications/test', method: 'post', data: { notify } })
}