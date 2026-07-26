<script setup>
import { computed, onMounted, ref } from "vue";
import { ElMessage, ElMessageBox } from "element-plus";
import { useI18n } from "vue-i18n";
import request from "@/utils/request";

const { t } = useI18n();

const currentPath = ref("/");
const pathHistory = ref(["/"]);
const loading = ref(false);
const fileList = ref([]);
const selectedFile = ref(null);

const createDialog = ref(false);
const createType = ref("file");
const createName = ref("");

const renameDialog = ref(false);
const renameFrom = ref("");
const renameTo = ref("");

const editDialog = ref(false);
const editPath = ref("");
const editContent = ref("");
const editLoading = ref(false);

const uploadDialog = ref(false);
const uploadRef = ref();

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

function formatSize(bytes) {
  if (bytes === 0) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  const i = Math.floor(Math.log(bytes) / Math.log(1024));
  return (bytes / Math.pow(1024, i)).toFixed(i > 0 ? 1 : 0) + " " + units[i];
}

function formatTime(ts) {
  if (!ts) return "-";
  return new Date(ts * 1000).toLocaleString();
}

async function loadFiles() {
  loading.value = true;
  try {
    const res = await request({
      url: "/svr/files/list",
      params: { path: currentPath.value },
      method: "get",
    });
    fileList.value = res.data || [];
  } catch {
    ElMessage.error(t("common.error"));
  } finally {
    loading.value = false;
  }
}

function navigateTo(path) {
  currentPath.value = path;
  pathHistory.value.push(path);
  loadFiles();
}

function goUp() {
  if (currentPath.value === "/") return;
  const parent = currentPath.value.substring(0, currentPath.value.lastIndexOf("/")) || "/";
  navigateTo(parent);
}

function breadcrumbClick(path) {
  navigateTo(path);
}

function handleRowClick(row) {
  if (row.is_dir) {
    navigateTo(row.path);
  } else {
    selectedFile.value = row;
  }
}

function handleRowDblclick(row) {
  if (row.is_dir) {
    navigateTo(row.path);
  } else {
    editFile(row.path);
  }
}

function openCreate(type) {
  createType.value = type;
  createName.value = "";
  createDialog.value = true;
}

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
      data: { path: fullPath },
    });
    ElMessage.success(createType.value === "dir" ? t("files.dirCreated") : t("files.fileCreated"));
    createDialog.value = false;
    loadFiles();
  } catch {
    ElMessage.error(t("common.error"));
  } finally {
    loading.value = false;
  }
}

async function editFile(path) {
  editLoading.value = true;
  editPath.value = path;
  editDialog.value = true;
  try {
    const res = await request({
      url: "/svr/files/read",
      params: { path },
      method: "get",
    });
    editContent.value = res.data || "";
  } catch {
    ElMessage.error(t("files.readFail"));
  } finally {
    editLoading.value = false;
  }
}

async function saveFile() {
  editLoading.value = true;
  try {
    await request({
      url: "/svr/files/write",
      method: "post",
      data: { path: editPath.value, content: editContent.value },
    });
    ElMessage.success(t("files.saved"));
    editDialog.value = false;
  } catch {
    ElMessage.error(t("files.saveFail"));
  } finally {
    editLoading.value = false;
  }
}

function openRename(path) {
  renameFrom.value = path;
  const name = path.split("/").pop();
  renameTo.value = name;
  renameDialog.value = true;
}

async function submitRename() {
  if (!renameTo.value.trim()) return;
  loading.value = true;
  try {
    const parent = renameFrom.value.substring(0, renameFrom.value.lastIndexOf("/")) || "";
    const newPath = parent + "/" + renameTo.value.trim();
    await request({
      url: "/svr/files/rename",
      method: "post",
      data: { from: renameFrom.value, to: newPath },
    });
    ElMessage.success(t("files.renamed"));
    renameDialog.value = false;
    loadFiles();
  } catch {
    ElMessage.error(t("files.renameFail"));
  } finally {
    loading.value = false;
  }
}

async function deleteFile(path) {
  try {
    await ElMessageBox.confirm(t("files.deleteConfirm", { path }), t("common.warning"), {
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
      data: { path },
    });
    ElMessage.success(t("files.deleted"));
    loadFiles();
  } catch {
    ElMessage.error(t("files.deleteFail"));
  } finally {
    loading.value = false;
  }
}

function downloadFile(path) {
  const name = path.split("/").pop();
  const baseUrl = import.meta.env.BASE_URL || "/";
  const url = baseUrl + "svr/files/download?path=" + encodeURIComponent(path);
  const a = document.createElement("a");
  a.href = url;
  a.download = name;
  a.click();
}

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
      headers: { "Content-Type": "multipart/form-data" },
      data: formData,
    });
    ElMessage.success(t("files.uploaded"));
    uploadDialog.value = false;
    loadFiles();
  } catch {
    ElMessage.error(t("files.uploadFail"));
  } finally {
    loading.value = false;
  }
}

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
      <div class="top-box-title">{{ t("files.title") }}</div>
    </div>

    <div class="toolbar">
      <div class="toolbar-left">
        <el-button size="small" @click="goUp" :disabled="currentPath === '/'">
          <el-icon><ArrowUp /></el-icon> {{ t("files.parentDir") }}
        </el-button>
        <el-button size="small" type="primary" @click="openCreate('file')">
          <el-icon><DocumentAdd /></el-icon> {{ t("files.newFile") }}
        </el-button>
        <el-button size="small" type="primary" @click="openCreate('dir')">
          <el-icon><FolderAdd /></el-icon> {{ t("files.newDir") }}
        </el-button>
        <el-button size="small" type="success" @click="uploadDialog = true">
          <el-icon><Upload /></el-icon> {{ t("files.upload") }}
        </el-button>
        <el-button size="small" @click="loadFiles">
          <el-icon><Refresh /></el-icon> {{ t("files.refresh") }}
        </el-button>
      </div>
    </div>

    <div class="breadcrumb">
      <template v-for="(crumb, idx) in breadcrumbs" :key="crumb.path">
        <span v-if="idx > 0" class="separator">/</span>
        <span class="crumb-item" @click="breadcrumbClick(crumb.path)">{{ crumb.name }}</span>
      </template>
    </div>

    <div class="file-table" v-loading="loading">
      <el-table :data="fileList" style="width: 100%" highlight-current-row
        @row-click="handleRowClick" @row-dblclick="handleRowDblclick" size="small">
        <el-table-column :label="t('files.name')" min-width="260">
          <template #default="{ row }">
            <div class="file-name-cell">
              <el-icon :size="18" :color="row.is_dir ? '#409EFF' : ''">
                <component :is="getFileIcon(row)" />
              </el-icon>
              <span :class="{ 'dir-name': row.is_dir }">{{ row.name }}</span>
            </div>
          </template>
        </el-table-column>
        <el-table-column :label="t('files.size')" width="120" align="right">
          <template #default="{ row }">
            {{ row.is_dir ? "-" : formatSize(row.size) }}
          </template>
        </el-table-column>
        <el-table-column :label="t('files.modified')" width="180">
          <template #default="{ row }">
            {{ formatTime(row.modified) }}
          </template>
        </el-table-column>
        <el-table-column :label="t('common.operate')" width="240" fixed="right">
          <template #default="{ row }">
            <el-button size="small" type="primary" link @click.stop="editFile(row.path)" v-if="!row.is_dir">{{ t("files.edit") }}</el-button>
            <el-button size="small" type="primary" link @click.stop="downloadFile(row.path)" v-if="!row.is_dir">{{ t("files.download") }}</el-button>
            <el-button size="small" type="warning" link @click.stop="openRename(row.path)">{{ t("files.rename") }}</el-button>
            <el-button size="small" type="danger" link @click.stop="deleteFile(row.path)">{{ t("files.delete") }}</el-button>
          </template>
        </el-table-column>
      </el-table>
      <el-empty v-if="!loading && fileList.length === 0" :description="t('files.empty')" :image-size="72" />
    </div>

    <el-dialog v-model="createDialog" :title="createType === 'dir' ? t('files.newDirTitle') : t('files.newFileTitle')" width="400px" :append-to-body="true">
      <el-input v-model="createName" :placeholder="createType === 'dir' ? t('files.dirNamePlaceholder') : t('files.fileNamePlaceholder')" @keyup.enter="submitCreate" />
      <template #footer>
        <el-button @click="createDialog = false">{{ t("common.cancel") }}</el-button>
        <el-button type="primary" @click="submitCreate">{{ t("common.confirm") }}</el-button>
      </template>
    </el-dialog>

    <el-dialog v-model="renameDialog" :title="t('files.renameTitle')" width="400px" :append-to-body="true">
      <el-input v-model="renameTo" :placeholder="t('files.renamePlaceholder')" @keyup.enter="submitRename" />
      <template #footer>
        <el-button @click="renameDialog = false">{{ t("common.cancel") }}</el-button>
        <el-button type="primary" @click="submitRename">{{ t("common.confirm") }}</el-button>
      </template>
    </el-dialog>

    <el-dialog v-model="editDialog" :title="t('files.editTitle', { path: editPath })" width="80%" top="3vh" :append-to-body="true">
      <div v-loading="editLoading" style="min-height: 200px">
        <el-input v-model="editContent" type="textarea" :rows="20" :placeholder="t('files.content')" />
      </div>
      <template #footer>
        <el-button @click="editDialog = false">{{ t("common.cancel") }}</el-button>
        <el-button type="primary" @click="saveFile" :loading="editLoading">{{ t("common.save") }}</el-button>
      </template>
    </el-dialog>

    <el-dialog v-model="uploadDialog" :title="t('files.uploadTitle')" width="400px" :append-to-body="true">
      <div>{{ t("files.uploadTo", { path: currentPath }) }}</div>
      <input type="file" ref="uploadRef" style="margin-top: 12px" />
      <template #footer>
        <el-button @click="uploadDialog = false">{{ t("common.cancel") }}</el-button>
        <el-button type="primary" @click="submitUpload">{{ t("files.upload") }}</el-button>
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
      background: var(--sidebar-hover);
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