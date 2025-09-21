/**
 * 日志数据接口定义
 */
interface LogEntry {
  time: number;
  level: string;
  message: string;
  module: string;
  function: string;
  line: number;
  process: number;
  thread: number;
  process_name: string;
  thread_name: string;
}

/**
 * 分页信息接口定义
 */
interface PaginationInfo {
  current_page: number;
  page_size: number;
  total_pages: number;
  total_logs: number;
}

/**
 * 日志数据响应接口定义
 */
interface LogsResponse {
  code: number;
  data: {
    logs: LogEntry[];
    pagination: PaginationInfo;
  };
  message?: string;
}

/**
 * 日志查看器类
 * 负责从 /api/logs 接口获取日志数据并显示在页面上
 */
class LogViewer {
  // 当前页码
  private currentPage: number = 1;
  // 每页显示的日志数量
  private pageSize: number = 20;
  // 当前选择的日志级别过滤器
  private currentLevelFilter: string = 'all';
  // 日志数据
  private logsData: LogEntry[] = [];
  // 分页信息
  private paginationInfo: PaginationInfo | null = null;

  // 页面元素引用
  private logsTableBody: HTMLElement | null = null;
  private refreshLogsButton: HTMLButtonElement | null = null;
  private toggleAutoRefreshButton: HTMLButtonElement | null = null;
  private logLevelFilter: HTMLSelectElement | null = null;
  private totalLogsCountElement: HTMLElement | null = null;
  private currentPageElement: HTMLElement | null = null;
  private totalPagesElement: HTMLElement | null = null;
  private pageInfoElement: HTMLElement | null = null;
  private prevPageButton: HTMLButtonElement | null = null;
  private nextPageButton: HTMLButtonElement | null = null;
  private logsContainer: HTMLElement | null = null;

  // 自动刷新相关属性
  private autoRefreshInterval: number = 1000; // 默认5秒刷新一次
  private autoRefreshTimer: number | null = null; // 自动刷新定时器ID
  private isAutoRefreshEnabled: boolean = true; // 是否启用自动刷新

  // WebUI日志过滤相关属性
  private showWebLogs: boolean = false; // 是否显示WebUI日志，默认不显示
  private toggleWebLogsButton: HTMLButtonElement | null = null; // WebUI日志开关按钮

  // 搜索相关属性
  private searchInput: HTMLInputElement | null = null; // 搜索输入框
  private searchButton: HTMLButtonElement | null = null; // 搜索按钮
  private currentSearchTerm: string = ''; // 当前搜索关键词

  /**
   * 构造函数，初始化日志查看器
   */
  constructor() {
    // 获取页面元素引用
    this.logsTableBody = document.getElementById('logs-table-body');
    this.refreshLogsButton = document.getElementById('refresh-logs-button') as HTMLButtonElement;
    this.toggleAutoRefreshButton = document.getElementById('toggle-auto-refresh-button') as HTMLButtonElement;
    this.logLevelFilter = document.getElementById('log-level-filter') as HTMLSelectElement;
    this.totalLogsCountElement = document.getElementById('total-logs-count');
    this.currentPageElement = document.getElementById('current-page');
    this.totalPagesElement = document.getElementById('total-pages');
    this.pageInfoElement = document.getElementById('page-info');
    this.prevPageButton = document.getElementById('prev-page') as HTMLButtonElement;
    this.nextPageButton = document.getElementById('next-page') as HTMLButtonElement;
    this.logsContainer = document.getElementById('logs-container');
    this.toggleWebLogsButton = document.getElementById('toggle-web-logs-button') as HTMLButtonElement;

    // 获取搜索元素引用
    this.searchInput = document.getElementById('log-search-input') as HTMLInputElement;
    this.searchButton = document.getElementById('search-logs-button') as HTMLButtonElement;

    // 设置事件监听器
    this.initEventListeners();

    // 初始加载日志数据
    this.fetchLogs();

    // 启动自动刷新
    this.startAutoRefresh();

    // 监听页面隐藏/显示事件，优化自动刷新
    document.addEventListener('visibilitychange', () => {
      if (document.hidden) {
        this.pauseAutoRefresh();
      } else if (this.isAutoRefreshEnabled) {
        this.resumeAutoRefresh();
      }
    });
  }

  /**
   * 初始化事件监听器
   */
  private initEventListeners(): void {
    // 刷新按钮点击事件
    if (this.refreshLogsButton) {
      this.refreshLogsButton.addEventListener('click', () => this.refreshLogs());
    }

    // 自动刷新切换按钮点击事件
    if (this.toggleAutoRefreshButton) {
      this.toggleAutoRefreshButton.addEventListener('click', () => this.toggleAutoRefresh());
    }

    // 日志级别过滤器变更事件
    if (this.logLevelFilter) {
      this.logLevelFilter.addEventListener('change', () => this.handleLevelFilterChange());
    }

    // 上一页按钮点击事件
    if (this.prevPageButton) {
      this.prevPageButton.addEventListener('click', () => this.goToPrevPage());
    }

    // 下一页按钮点击事件
    if (this.nextPageButton) {
      this.nextPageButton.addEventListener('click', () => this.goToNextPage());
    }

    // WebUI日志开关事件监听
    if (this.toggleWebLogsButton) {
      this.toggleWebLogsButton.addEventListener('click', () => this.toggleWebLogs());
    }

    // 搜索按钮点击事件
    if (this.searchButton) {
      this.searchButton.addEventListener('click', () => this.performSearch());
    }

    // 搜索输入框键盘事件（按Enter键搜索）
    if (this.searchInput) {
      this.searchInput.addEventListener('keypress', (event) => {
        if (event.key === 'Enter') {
          this.performSearch();
        }
      });
    }
  }

  /**
   * 启动自动刷新
   */
  private startAutoRefresh(): void {
    this.isAutoRefreshEnabled = true;
    this.setupAutoRefreshTimer();
  }

  /**
   * 暂停自动刷新
   */
  private pauseAutoRefresh(): void {
    if (this.autoRefreshTimer !== null) {
      clearInterval(this.autoRefreshTimer);
      this.autoRefreshTimer = null;
    }
  }

  /**
   * 恢复自动刷新
   */
  private resumeAutoRefresh(): void {
    if (this.isAutoRefreshEnabled) {
      this.setupAutoRefreshTimer();
    }
  }

  /**
   * 设置自动刷新定时器
   */
  private setupAutoRefreshTimer(): void {
    // 清除已有的定时器
    this.pauseAutoRefresh();

    // 创建新的定时器
    this.autoRefreshTimer = window.setInterval(() => {
      this.fetchLogs();
    }, this.autoRefreshInterval);
  }

  /**
   * 切换WebUI日志显示状态
   */
  private toggleWebLogs(): void {
    // 切换状态
    this.showWebLogs = !this.showWebLogs;

    // 刷新当前页面的日志数据
    this.fetchLogs();

    // 更新按钮显示文本
    if (this.toggleWebLogsButton) {
      // 添加/移除按钮的激活状态类
      if (this.showWebLogs) {
        this.toggleWebLogsButton.classList.add('active');
      } else {
        this.toggleWebLogsButton.classList.remove('active');
      }
    }

    // 更新日志显示
    this.updateLogsDisplay();
  }

  /**
   * 执行日志搜索
   */
  private performSearch(): void {
    if (!this.searchInput) return;

    // 获取搜索关键词（去除首尾空格）
    this.currentSearchTerm = this.searchInput.value.trim();

    // 更新日志显示（应用新的搜索条件）
    this.updateLogsDisplay();
  }

  /**
   * 切换自动刷新状态
   */
  private toggleAutoRefresh(): void {
    if (this.isAutoRefreshEnabled) {
      this.pauseAutoRefresh();
      this.isAutoRefreshEnabled = false;
    } else {
      this.startAutoRefresh();
    }

    // 更新按钮显示文本
    if (this.toggleAutoRefreshButton) { // 不知道为什么逻辑要反过来
      // 添加/移除按钮的激活状态类
      if (this.isAutoRefreshEnabled) {
        this.toggleAutoRefreshButton.classList.remove('active');
      } else {
        this.toggleAutoRefreshButton.classList.add('active');
      }
    }
  }

  /**
   * 从 /api/logs 接口获取日志数据
   */
  private async fetchLogs(): Promise<void> {
    try {
      // 构建请求URL
      const url = `/api/logs?page=${this.currentPage}&page_size=${this.pageSize}&no_webui=${!this.showWebLogs}`;
      const response = await fetch(url);

      if (!response.ok) {
        throw new Error(`HTTP error! status: ${response.status}`);
      }

      const logsResponse = await response.json() as LogsResponse;

      if (logsResponse.code === 0 && logsResponse.data) {
        // 将日志按照时间倒序排序，最新的在最上面
        this.logsData = logsResponse.data.logs.sort((a, b) => b.time - a.time);
        this.paginationInfo = logsResponse.data.pagination;
        this.updateLogsDisplay();
      } else {
        throw new Error(logsResponse.message || 'Failed to fetch logs');
      }
    } catch (error) {
      console.error('Error fetching logs:', error);
      this.showErrorMessage('获取日志数据失败');
    }
  }

  /**
   * 更新日志显示
   */
  private updateLogsDisplay(): void {

    // 过滤日志数据
    const filteredLogs = this.filterLogsByLevel(this.logsData);

    // 更新表格内容
    this.updateLogsTable(filteredLogs);

    // 更新分页信息
    this.updatePaginationInfo();

    // 更新统计信息
    this.updateStats();

    // 检查是否有空日志情况
    if (filteredLogs.length === 0) {
      this.showEmptyState();
    } else {
      this.hideEmptyState();
    }
  }

  /**
   * 根据日志级别、WebUI日志和搜索关键词过滤日志数据
   * @param logs 原始日志数据
   * @returns 过滤后的日志数据
   */
  private filterLogsByLevel(logs: LogEntry[]): LogEntry[] {
    let filteredLogs = logs;

    // 先按日志级别过滤
    if (this.currentLevelFilter !== 'all') {
      filteredLogs = filteredLogs.filter(log => log.level === this.currentLevelFilter);
    }

    // 最后应用搜索过滤（如果有搜索关键词）
    if (this.currentSearchTerm) {
      // 将空格分割的多个关键词转换为数组（去除空字符串）
      const keywords = this.currentSearchTerm.split(/\s+/).filter(keyword => keyword.length > 0);

      // 如果有多个关键词，则应用逻辑与（AND）过滤
      if (keywords.length > 0) {
        filteredLogs = filteredLogs.filter(log => {
          // 对每条日志，检查是否包含所有关键词
          return keywords.every(keyword => {
            // 关键词需要匹配日志的message、module或function字段
            const searchText = `${log.message || ''} ${log.module || ''} ${log.function || ''}`.toLowerCase();
            return searchText.includes(keyword.toLowerCase());
          });
        });
      }
    }

    return filteredLogs;
  }

  /**
   * 更新日志表格
   * @param logs 要显示的日志数据
   */
  private updateLogsTable(logs: LogEntry[]): void {
    if (!this.logsTableBody) return;

    // 清空表格
    this.logsTableBody.innerHTML = '';

    // 添加日志行
    logs.forEach(log => {
      const row = document.createElement('tr');
      
      // 为日志行添加点击事件，显示完整信息
      row.addEventListener('click', () => this.showLogDetails(log));
      // 添加样式以便用户知道这是可点击的
      row.classList.add('log-row');

      // 格式化时间
      const formattedTime = new Date(log.time * 1000).toLocaleString('zh-CN', {
        year: 'numeric',
        month: '2-digit',
        day: '2-digit',
        hour: '2-digit',
        minute: '2-digit',
        second: '2-digit'
      });

      // 创建表格单元格
      row.innerHTML = `
        <td>${formattedTime}</td>
        <td>
          <span class="log-level ${log.level}">${log.level}</span>
        </td>
        <td>${log.message}</td>
        <td>${log.module}</td>
        <td>${log.function}</td>
      `;

      // 添加到表格
      if (this.logsTableBody) {
        this.logsTableBody.appendChild(row);
      }
    });
  }

  /**
   * 显示日志完整信息
   * @param log 日志条目对象
   */
  private showLogDetails(log: LogEntry): void {
    // 格式化时间（不使用fractionalSecondDigits属性以兼容当前TypeScript配置）
    const formattedTime = new Date(log.time * 1000).toLocaleString('zh-CN', {
      year: 'numeric',
      month: '2-digit',
      day: '2-digit',
      hour: '2-digit',
      minute: '2-digit',
      second: '2-digit'
    });
    
    // 创建包含完整日志信息的HTML内容，使用卡片式布局替代表格
    const detailsContent = document.createElement('div');
    detailsContent.className = 'log-details-container';
    detailsContent.innerHTML = `
      <div class="log-details-grid">
        <div class="log-detail-item">
          <div class="log-detail-label">时间：</div>
          <div class="log-detail-value">${formattedTime}</div>
        </div>
        <div class="log-detail-item">
          <div class="log-detail-label">级别：</div>
          <div class="log-detail-value"><span class="log-level ${log.level}">${log.level}</span></div>
        </div>
        <div class="log-detail-item log-message-item">
          <div class="log-detail-label">消息：</div>
          <div class="log-detail-value">${log.message}</div>
        </div>
        <div class="log-detail-item">
          <div class="log-detail-label">模块：</div>
          <div class="log-detail-value">${log.module}</div>
        </div>
        <div class="log-detail-item">
          <div class="log-detail-label">函数：</div>
          <div class="log-detail-value">${log.function}</div>
        </div>
        <div class="log-detail-item">
          <div class="log-detail-label">行号：</div>
          <div class="log-detail-value">${log.line}</div>
        </div>
        <div class="log-detail-item">
          <div class="log-detail-label">进程ID：</div>
          <div class="log-detail-value">${log.process}</div>
        </div>
        <div class="log-detail-item">
          <div class="log-detail-label">线程ID：</div>
          <div class="log-detail-value">${log.thread}</div>
        </div>
        <div class="log-detail-item">
          <div class="log-detail-label">进程名称：</div>
          <div class="log-detail-value">${log.process_name}</div>
        </div>
        <div class="log-detail-item">
          <div class="log-detail-label">线程名称：</div>
          <div class="log-detail-value">${log.thread_name}</div>
        </div>
      </div>
    `;
    
    // 使用之前实现的alert系统显示日志详情
    showAlert('日志详情', detailsContent, 'info', false);
  }

  /**
   * 更新分页信息
   */
  private updatePaginationInfo(): void {
    if (!this.paginationInfo) return;

    // 更新分页控件状态
    if (this.prevPageButton) {
      this.prevPageButton.disabled = this.currentPage <= 1;
    }

    if (this.nextPageButton) {
      this.nextPageButton.disabled = this.currentPage >= this.paginationInfo.total_pages;
    }

    // 更新分页信息文本
    if (this.pageInfoElement) {
      this.pageInfoElement.textContent = `第 ${this.currentPage} 页，共 ${this.paginationInfo.total_pages} 页`;
    }

    // 更新统计信息
    if (this.totalLogsCountElement) {
      this.totalLogsCountElement.textContent = this.paginationInfo.total_logs.toString();
    }

    if (this.currentPageElement) {
      this.currentPageElement.textContent = this.currentPage.toString();
    }

    if (this.totalPagesElement) {
      this.totalPagesElement.textContent = this.paginationInfo.total_pages.toString();
    }
  }

  /**
   * 更新统计信息
   */
  private updateStats(): void {
    // 这里可以添加更多统计信息的更新逻辑
  }

  /**
   * 处理日志级别过滤器变更
   */
  private handleLevelFilterChange(): void {
    if (this.logLevelFilter) {
      this.currentLevelFilter = this.logLevelFilter.value;
      this.currentPage = 1; // 重置为第一页
      this.fetchLogs(); // 重新获取日志数据
    }
  }

  /**
   * 刷新日志数据
   */
  private refreshLogs(): void {
    this.fetchLogs();
  }

  /**
   * 跳转到上一页
   */
  private goToPrevPage(): void {
    if (this.currentPage > 1) {
      this.currentPage--;
      this.fetchLogs();
    }
  }

  /**
   * 跳转到下一页
   */
  private goToNextPage(): void {
    if (this.paginationInfo && this.currentPage < this.paginationInfo.total_pages) {
      this.currentPage++;
      this.fetchLogs();
    }
  }

  /**
   * 显示空状态
   */
  private showEmptyState(): void {
    if (!this.logsContainer) return;

    // 隐藏表格
    if (this.logsTableBody && this.logsTableBody.parentElement) {
      this.logsTableBody.parentElement.style.display = 'none';
    }

    // 检查是否已存在空状态元素
    let emptyElement = document.getElementById('empty-logs');
    if (!emptyElement) {
      emptyElement = document.createElement('div');
      emptyElement.id = 'empty-logs';
      emptyElement.innerHTML = '<i class="fas fa-file-alt"></i><p>暂无日志数据</p>';
      this.logsContainer.appendChild(emptyElement);
    } else {
      emptyElement.style.display = 'block';
    }
  }

  /**
   * 隐藏空状态
   */
  private hideEmptyState(): void {
    const emptyElement = document.getElementById('empty-logs');
    if (emptyElement) {
      emptyElement.style.display = 'none';
    }
  }

  /**
   * 显示错误消息
   * @param message 错误消息内容
   */
  private showErrorMessage(message: string): void {
    if (!this.logsContainer) return;

    // 隐藏表格和加载状态
    if (this.logsTableBody && this.logsTableBody.parentElement) {
      this.logsTableBody.parentElement.style.display = 'none';
    }
    this.hideEmptyState();

    // 创建错误消息元素
    let errorElement = document.createElement('div');
    errorElement.className = 'error-container';
    errorElement.style.marginTop = '20px';
    errorElement.innerHTML = `<p><i class="fas fa-exclamation-circle"></i> ${message}</p>`;

    // 清除之前的错误消息
    const oldErrorElement = this.logsContainer.querySelector('.error-container');
    if (oldErrorElement) {
      this.logsContainer.removeChild(oldErrorElement);
    }

    // 添加新的错误消息
    this.logsContainer.appendChild(errorElement);

    // 3秒后自动移除错误消息
    setTimeout(() => {
      if (errorElement && errorElement.parentElement) {
        errorElement.parentElement.removeChild(errorElement);
        this.showEmptyState();
      }
    }, 3000);
  }
}

/**
 * 立即初始化日志查看器
 * 不依赖DOMContentLoaded事件，确保在页面内容动态加载时也能正常初始化
 */
function initializeLogViewer() {
  // 检查是否所有必要的DOM元素都已加载
  const requiredElements = [
    'logs-table-body',
    'refresh-logs-button',
    'log-level-filter',
    'total-logs-count',
    'current-page',
    'total-pages',
    'page-info',
    'prev-page',
    'next-page',
    'logs-container'
  ];

  // 检查每个必要元素是否存在
  const allElementsExist = requiredElements.every(id => document.getElementById(id) !== null);

  // 如果所有元素都存在，初始化LogViewer
  if (allElementsExist) {
    new LogViewer();
  } else {
    // 如果有元素不存在，延迟一小段时间后重试
    setTimeout(initializeLogViewer, 100);
  }
}

// 立即尝试初始化
initializeLogViewer();
