import request from '@/utils/request'
import type { ApiResponse, PageData, Job, TaskInfo, TaskItem } from './types'

// ========== 作业 CRUD ==========

// GET /api/jobs
export function listJobs(params: { pageNum?: number; pageSize?: number } = {}) {
  return request<ApiResponse<PageData<Job>>>({ url: '/api/jobs', method: 'get', params })
}

// POST /api/jobs
export function createJob(data: Record<string, unknown>) {
  return request<ApiResponse<null>>({ url: '/api/jobs', method: 'post', data })
}

// PUT /api/jobs/:id
export function updateJob(id: number, data: Record<string, unknown>) {
  return request<ApiResponse<null>>({ url: `/api/jobs/${id}`, method: 'put', data })
}

// DELETE /api/jobs/:id
export function deleteJob(id: number) {
  return request<ApiResponse<null>>({ url: `/api/jobs/${id}`, method: 'delete' })
}

// ========== 作业操作 ==========

// POST /api/jobs/:id/run
export function runJob(id: number) {
  return request<ApiResponse<null>>({ url: `/api/jobs/${id}/run`, method: 'post' })
}

// POST /api/jobs/:id/pause
export function pauseJob(id: number) {
  return request<ApiResponse<null>>({ url: `/api/jobs/${id}/pause`, method: 'post' })
}

// POST /api/jobs/:id/resume
export function resumeJob(id: number) {
  return request<ApiResponse<null>>({ url: `/api/jobs/${id}/resume`, method: 'post' })
}

// POST /api/jobs/:id/abort
export function abortJob(id: number) {
  return request<ApiResponse<null>>({ url: `/api/jobs/${id}/abort`, method: 'post' })
}

// POST /api/jobs/run-all
export function runAllJobs() {
  return request<ApiResponse<null>>({ url: `/api/jobs/run-all`, method: 'post' })
}

// ========== 任务查询 ==========

// GET /api/jobs/:id/current
export function getJobCurrent(id: number, status?: number) {
  return request<ApiResponse<TaskInfo | null>>({
    url: `/api/jobs/${id}/current`,
    method: 'get',
    params: status !== undefined ? { status } : undefined,
  })
}

// GET /api/jobs/:id/tasks
export function getJobTasks(id: number, params: { pageNum?: number; pageSize?: number } = {}) {
  return request<ApiResponse<PageData<TaskInfo>>>({
    url: `/api/jobs/${id}/tasks`,
    method: 'get',
    params,
  })
}

// DELETE /api/tasks/:id
export function deleteTask(id: number) {
  return request<ApiResponse<null>>({ url: `/api/tasks/${id}`, method: 'delete' })
}

// GET /api/tasks/:id/items
export function getTaskItems(id: number, params: { pageNum?: number; pageSize?: number } = {}) {
  return request<ApiResponse<PageData<TaskItem>>>({
    url: `/api/tasks/${id}/items`,
    method: 'get',
    params,
  })
}