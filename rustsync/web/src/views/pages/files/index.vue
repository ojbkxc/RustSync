<script setup>
import { computed, onMounted, ref } from "vue";
import { ElMessage, ElMessageBox } from "element-plus";
import { useI18n } from "vue-i18n";
import request from "@/utils/request";

const { t } = useI18n();

// 当前路径
const currentPath = ref("/");
const pathHistory = ref(["/"]);
const loading = ref(false);
const fileList = ref([]);
const selectedFile = ref(null);

// 新建对话框
const createDialog = ref(false);
const createType = ref("file");
const createName = ref("");

// 重命名对话框
const renameDialog = ref(false);
const renameFrom = ref("");
const renameTo = ref("");

// 文件内容查看/编辑
const editDialog = ref(false);
const editPath = ref("");
const editContent = ref("");
const editLoading = ref(false);

// 上传
const uploadDialog = ref(false);
const uploadRef = ref();

// 面包屑
const breadcrumbs = computed(() => {
  const parts = currentPath.value.split("/").filter(Boolean);
  const crumbs = [{ name: "/", path: "/" }];
  let accumulated = "";
  for (const part of parts) {
    accumulated += "/" + part;
    crumbs.push({ name: part, path: accumulated });
  }
  return crumbs;
});

// 格式化文件大小
function formatSize(bytes) {
  if (bytes === 0) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  const i = Math.floor(Math.log(bytes) / Math.log(1024));
  return (bytes / Math.pow(1024, i)).toFixed(i > 0 ? 1 : 0) + " " + units[i];
}

// 格式化时间
function formatTime(ts) {
  if (!ts) return "-";
  return new Date(ts * 1000).toLocaleString();
}

// 获取文件列表
async function loadFiles() {
  loading.value = true;
  try {
    const res = await request({
      url: "/svr/files/list",
      params: { path: currentPath.value },
      headers: { isMask: false },
      method: "get",
    });
    fileList.value = res.data || [];
  } catch {
    ElMessage.error(t("common.error"));
  } finally {
    loading.value = false;
  }
}

// 导航到目录
function navigateTo(path) {
  currentPath.value = path;
  pathHistory.value.push(path);
  loadFiles();
}

// 返回上级
function goUp() {
  if (currentPath.value === "/") return;
  const parent = currentPath.value.substring(0, currentPath.value.lastIndexOf("/")) || "/";
  navigateTo(parent);
}

// 面包屑点击
function breadcrumbClick(path) {
  navigateTo(path);
}

// 点击文件/目录
function handleRowClick(row) {
  if (row.is_dir) {
    navigateTo(row.path);
  } else {
    selectedFile.value = row;
  }
}

// 双击编辑
function handleRowDblclick(row) {
  if (row.is_dir) {
    navigateTo(row.path);
  } else {
    editFile(row.path);
  }
}

// 打开创建对话框
function openCreate(type) {
  createType.value = type;
  createName.value = "";
  createDialog.value = true;
}

// 提交创建
async function submitCreate() {
  if (!createName.value.trim()) return;
  loading.value = true;
  try {
    const fullPath = currentPath.value === "/"
      ? "/" + createName.value.trim()
      : currentPath.value + "/" + createName.value.trim();
    const url = createType.value === "dir" ? "/svr/files/mkdir" : "/svr/files/touch";
    await request({
      url,
      method: "post",
      headers: { isMask: false },
      data: { path: fullPath },
    });
    ElMessage.success(createType.value === "dir" ? "目录已创建" : "文件已创建");
    createDialog.value = false;
    loadFiles();
  } catch {
    ElMessage.error(t("common.error"));
  } finally {
    loading.value = false;
  }
}

// 编辑文件
async function editFile(path) {
  editLoading.value = true;
  editPath.value = path;
  editDialog.value = true;
  try {
    const res = await request({
      url: "/svr/files/read",
      params: { path },
      headers: { isMask: false },
      method: "get",
    });
    editContent.value = res.data || "";
  } catch {
    ElMessage.error("读取文件失败");
  } finally {
    editLoading.value = false;
  }
}

// 保存文件
async function saveFile() {
  editLoading.value = true;
  try {
    await request({
      url: "/svr/files/write",
      method: "post",
      headers: { isMask: false },
      data: { path: editPath.value, content: editContent.value },
    });
    ElMessage.success("文件已保存");
    editDialog.value = false;
  } catch {
    ElMessage.error("保存失败");
  } finally {
    editLoading.value = false;
  }
}

// 打开重命名对话框
function openRename(path) {
  renameFrom.value = path;
  const name = path.split("/").pop();
  renameTo.value = name;
  renameDialog.value = true;
}

// 提交重命名
async function submitRename() {
  if (!renameTo.value.trim()) return;
  loading.value = true;
  try {
    const parent = renameFrom.value.substring(0, renameFrom.value.lastIndexOf("/")) || "";
    const newPath = parent + "/" + renameTo.value.trim();
    await request({
      url: "/svr/files/rename",
      method: "post",
      headers: { isMask: false },
      data: { from: renameFrom.value, to: newPath },
    });
    ElMessage.success("已重命名");
    renameDialog.value = false;
    loadFiles();
  } catch {
    ElMessage.error("重命名失败");
  } finally {
    loading.value = false;
  }
}

// 删除文件
async function deleteFile(path) {
  try {
    await ElMessageBox.confirm("确定删除 " + path + " 吗？此操作不可恢复。", t("common.warning"), {
      confirmButtonText: t("common.confirm"),
      cancelButtonText: t("common.cancel"),
      type: "warning",
    });
  } catch {
    return;
  }
  loading.value = true;
  try {
    await request({
      url: "/svr/files/delete",
      method: "post",
      headers: { isMask: false },
      data: { path },
    });
    ElMessage.success("已删除");
    loadFiles();
  } catch {
    ElMessage.error("删除失败");
  } finally {
    loading.value = false;
  }
}

// 下载文件
function downloadFile(path) {
  const name = path.split("/").pop();
  const baseUrl = import.meta.env.BASE_URL || "/";
  const url = baseUrl + "svr/files/download?path=" + encodeURIComponent(path);
  const a = document.createElement("a");
  a.href = url;
  a.download = name;
  a.click();
}

// 上传文件
async function submitUpload() {
  const file = uploadRef.value?.files?.[0];
  if (!file) return;
  loading.value = true;
  try {
    const formData = new FormData();
    formData.append("dir", currentPath.value);
    formData.append("file", file);
    await request({
      url: "/svr/files/upload",
      method: "post",
      headers: { isMask: false, "Content-Type": "multipart/form-data" },
      data: formData,
    });
    ElMessage.success("上传成功");
    uploadDialog.value = false;
    loadFiles();
  } catch {
    ElMessage.error("上传失败");
  } finally {
    loading.value = false;
  }
}

// 获取文件图标
function getFileIcon(row) {
  if (row.is_dir) return "Folder";
  const ext = row.extension?.toLowerCase();
  const iconMap = {
    txt: "Document", log: "Document", json: "Document", xml: "Document",
    html: "Document", css: "Document", js: "Document", ts: "Document",
    py: "Document", rs: "Document", java: "Document", kt: "Document",
    sh: "Document", bash: "Document",
    png: "Picture", jpg: "Picture", jpeg: "Picture", gif: "Picture",
    webp: "Picture", svg: "Picture",
    mp3: "VideoCamera", wav: "VideoCamera", ogg: "VideoCamera",
    mp4: "VideoCamera", webm: "VideoCamera", mkv: "VideoCamera",
    zip: "FolderOpened", gz: "FolderOpened", tar: "FolderOpened", rar: "FolderOpened",
    pdf: "Document",
    apk: "Document",
  };
  return iconMap[ext] || "Document";
}

onMounted(() => {
  loadFiles();
});
</script>

<template>
  <div class="files">
    <div class="top-box">
      <div class="top-box-title">文件管理器</div>
    </div>

    <!-- 工具栏 -->
    <div class="toolbar">
      <div class="toolbar-left">
        <el-button size="small" @click="goUp" :disabled="currentPath === '/'">
          <el-icon><ArrowUp /></el-icon> 上级目录
        </el-button>
        <el-button size="small" type="primary" @click="openCreate('file')">
          <el-icon><DocumentAdd /></el-icon> 新建文件
        </el-button>
        <el-button size="small" type="primary" @click="openCreate('dir')">
          <el-icon><FolderAdd /></el-icon> 新建目录
        </el-button>
        <el-button size="small" type="success" @click="uploadDialog = true">
          <el-icon><Upload /></el-icon> 上传
        </el-button>
        <el-button size="small" @click="loadFiles">
          <el-icon><Refresh /></el-icon> 刷新
        </el-button>
      </div>
    </div>

    <!-- 面包屑 -->
    <div class="breadcrumb">
      <template v-for="(crumb, idx) in breadcrumbs" :key="crumb.path">
        <span v-if="idx > 0" class="separator">/</span>
        <span class="crumb-item" @click="breadcrumbClick(crumb.path)">{{ crumb.name }}</span>
      </template>
    </div>

    <!-- 文件列表 -->
    <div class="file-table" v-loading="loading">
      <el-table :data="fileList" style="width: 100%" highlight-current-row
        @row-click="handleRowClick" @row-dblclick="handleRowDblclick" size="small">
        <el-table-column label="名称" min-width="260">
          <template #default="{ row }">
            <div class="file-name-cell">
              <el-icon :size="18" :color="row.is_dir ? '#409EFF' : ''">
                <component :is="getFileIcon(row)" />
              </el-icon>
              <span :class="{ 'dir-name': row.is_dir }">{{ row.name }}</span>
            </div>
          </template>
        </el-table-column>
        <el-table-column label="大小" width="120" align="right">
          <template #default="{ row }">
            {{ row.is_dir ? "-" : formatSize(row.size) }}
          </template>
        </el-table-column>
        <el-table-column label="修改时间" width="180">
          <template #default="{ row }">
            {{ formatTime(row.modified) }}
          </template>
        </el-table-column>
        <el-table-column label="操作" width="240" fixed="right">
          <template #default="{ row }">
            <el-button size="small" type="primary" link @click.stop="editFile(row.path)" v-if="!row.is_dir">编辑</el-button>
            <el-button size="small" type="primary" link @click.stop="downloadFile(row.path)" v-if="!row.is_dir">下载</el-button>
            <el-button size="small" type="warning" link @click.stop="openRename(row.path)">重命名</el-button>
            <el-button size="small" type="danger" link @click.stop="deleteFile(row.path)">删除</el-button>
          </template>
        </el-table-column>
      </el-table>
      <div v-if="!loading && fileList.length === 0" class="empty-state">此目录为空</div>
    </div>

    <!-- 创建对话框 -->
    <el-dialog v-model="createDialog" :title="createType === 'dir' ? '新建目录' : '新建文件'" width="400px" :append-to-body="true">
      <el-input v-model="createName" :placeholder="createType === 'dir' ? '请输入目录名' : '请输入文件名'" @keyup.enter="submitCreate" />
      <template #footer>
        <el-button @click="createDialog = false">取消</el-button>
        <el-button type="primary" @click="submitCreate">确定</el-button>
      </template>
    </el-dialog>

    <!-- 重命名对话框 -->
    <el-dialog v-model="renameDialog" title="重命名" width="400px" :append-to-body="true">
      <el-input v-model="renameTo" placeholder="请输入新名称" @keyup.enter="submitRename" />
      <template #footer>
        <el-button @click="renameDialog = false">取消</el-button>
        <el-button type="primary" @click="submitRename">确定</el-button>
      </template>
    </el-dialog>

    <!-- 编辑对话框 -->
    <el-dialog v-model="editDialog" :title="'编辑: ' + editPath" width="80%" top="3vh" :append-to-body="true">
      <div v-loading="editLoading" style="min-height: 200px">
        <el-input v-model="editContent" type="textarea" :rows="20" placeholder="文件内容" />
      </div>
      <template #footer>
        <el-button @click="editDialog = false">取消</el-button>
        <el-button type="primary" @click="saveFile" :loading="editLoading">保存</el-button>
      </template>
    </el-dialog>

    <!-- 上传对话框 -->
    <el-dialog v-model="uploadDialog" title="上传文件" width="400px" :append-to-body="true">
      <div>上传到: {{ currentPath }}</div>
      <input type="file" ref="uploadRef" style="margin-top: 12px" />
      <template #footer>
        <el-button @click="uploadDialog = false">取消</el-button>
        <el-button type="primary" @click="submitUpload">上传</el-button>
      </template>
    </el-dialog>
  </div>
</template>

<style lang="scss" scoped>
.files {
  width: 100%;
  height: 100%;
  box-sizing: border-box;
  display: flex;
  flex-direction: column;
  padding: 16px;
}

.top-box {
  margin-bottom: 12px;
  .top-box-title {
    font-size: 20px;
    font-weight: 600;
    color: var(--text-primary);
  }
}

.toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 10px;
  flex-wrap: wrap;
  gap: 8px;

  .toolbar-left {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-wrap: wrap;
  }
}

.breadcrumb {
  display: flex;
  align-items: center;
  padding: 8px 12px;
  background: var(--home-item-background-color);
  border-radius: 6px;
  margin-bottom: 10px;
  overflow-x: auto;
  white-space: nowrap;

  .crumb-item {
    color: var(--active-color);
    cursor: pointer;
    padding: 2px 4px;
    border-radius: 3px;
    &:hover {
      background: rgba(64, 158, 255, 0.1);
    }
  }

  .separator {
    margin: 0 4px;
    color: var(--text-muted);
  }
}

.file-table {
  flex: 1;
  overflow: auto;
  background: var(--home-item-background-color);
  border-radius: 6px;
  padding: 8px;
}

.file-name-cell {
  display: flex;
  align-items: center;
  gap: 8px;
  cursor: pointer;

  .dir-name {
    color: var(--active-color);
    font-weight: 500;
  }
}

.empty-state {
  text-align: center;
  padding: 40px;
  color: var(--text-muted);
  font-size: 14px;
}

@media (max-width: 768px) {
  .files {
    padding: 8px;
  }

  .toolbar {
    .toolbar-left {
      width: 100%;
      .el-button {
        font-size: 12px;
        padding: 5px 10px;
      }
    }
  }
}
</style>