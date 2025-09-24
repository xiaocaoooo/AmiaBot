/**
 * 用户信息管理类
 * 负责从 /api/self 接口获取用户信息并更新页面显示，同时获取一言数据、系统信息和插件信息
 */
class UserInfoManager {
  // 定义用户信息的接口类型
  private userInfo: UserInfo | null = null;
  private avatarElement: HTMLImageElement | null = null;
  private nameElement: HTMLElement | null = null;
  private idElement: HTMLElement | null = null;

  // 一言相关元素引用
  private hitokotoContentElement: HTMLElement | null = null;
  private hitokotoFromElement: HTMLElement | null = null;
  private hitokotoAuthorElement: HTMLElement | null = null;

  // 系统信息相关元素引用
  private systemInfoElement: HTMLElement | null = null;
  private cpuUsageElement: HTMLElement | null = null;
  private cpuUsageBarElement: HTMLElement | null = null;
  private memoryUsageElement: HTMLElement | null = null;
  private memoryUsageBarElement: HTMLElement | null = null;
  private memoryUsageBarProjectElement: HTMLElement | null = null;
  private diskUsageElement: HTMLElement | null = null;
  private diskUsageBarElement: HTMLElement | null = null;
  private uptimeElement: HTMLElement | null = null;
  private osElement: HTMLElement | null = null;
  private pythonVersionElement: HTMLElement | null = null;

  // 插件信息相关元素引用
  private pluginInfoElement: HTMLElement | null = null;
  private pluginsCountElement: HTMLElement | null = null;
  private enabledPluginsCountElement: HTMLElement | null = null;
  private reloadPluginsButton: HTMLButtonElement | null = null;

  /**
   * 构造函数，初始化用户信息管理器
   */
  constructor() {
    // 获取页面元素引用
    this.avatarElement = document.querySelector('#user-avatar img');
    this.nameElement = document.getElementById('user-name');
    this.idElement = document.getElementById('user-id');

    // 获取一言相关元素引用
    this.hitokotoContentElement = document.getElementById('hitokoto-content');
    this.hitokotoFromElement = document.getElementById('hitokoto-from');
    this.hitokotoAuthorElement = document.getElementById('hitokoto-author');

    // 获取系统信息元素引用
    this.systemInfoElement = document.getElementById('system-info');
    this.cpuUsageElement = document.getElementById('cpu-usage');
    this.cpuUsageBarElement = document.getElementById('cpu-usage-bar');
    this.memoryUsageElement = document.getElementById('memory-usage');
    this.memoryUsageBarElement = document.getElementById('memory-usage-bar');
    this.memoryUsageBarProjectElement = document.getElementById('memory-usage-bar-project');
    this.diskUsageElement = document.getElementById('disk-usage');
    this.diskUsageBarElement = document.getElementById('disk-usage-bar');
    this.uptimeElement = document.getElementById('uptime');
    this.osElement = document.getElementById('os');
    this.pythonVersionElement = document.getElementById('python-version');

    // 获取插件信息元素引用
    this.pluginInfoElement = document.getElementById('plugin-info');
    this.pluginsCountElement = document.getElementById('plugins-count');
    this.enabledPluginsCountElement = document.getElementById('enabled-plugins-count');
    this.reloadPluginsButton = document.getElementById('reload-plugins-button') as HTMLButtonElement;

    // 页面加载时获取用户信息
    this.fetchUserInfo();

    // 同时获取一言数据
    this.fetchHitokoto();

    // 获取系统信息并设置定时器定期更新
    this.fetchSystemInfo();
    setInterval(() => this.fetchSystemInfo(), 1000);

    // 设置重载插件按钮点击事件
    if (this.reloadPluginsButton) {
      this.reloadPluginsButton.addEventListener('click', () => this.reloadPlugins());
    }

    // 获取插件信息并设置定时器定期更新
    this.fetchPluginInfo();
    setInterval(() => this.fetchPluginInfo(), 5000); // 每5秒更新一次插件信息
  }

  /**
   * 从 /api/self 接口获取用户信息
   */
  private async fetchUserInfo(): Promise<void> {
    try {
      const response = await fetch('/api/self');
      if (!response.ok) {
        throw new Error(`HTTP error! status: ${response.status}`);
      }
      this.userInfo = await response.json() as UserInfo;
      this.updateUserInfoDisplay();
    } catch (error) {
      console.error('Failed to fetch user info:', error);
      this.displayErrorMessage('获取用户信息失败');
    }
  }

  /**
   * 更新页面上的用户信息显示
   */
  private updateUserInfoDisplay(): void {
    if (!this.userInfo) return;

    // 更新头像
    if (this.avatarElement && this.userInfo.avatar) {
      this.avatarElement.src = this.userInfo.avatar;
      this.avatarElement.alt = this.userInfo.nick || '用户头像';
    }

    // 更新用户名
    if (this.nameElement) {
      this.nameElement.textContent = this.userInfo.nick || '未知用户';
    }

    // 更新用户ID
    if (this.idElement) {
      this.idElement.textContent = `${this.userInfo.qq}`;
    }
  }

  /**
   * 显示错误信息
   * @param message 错误信息
   */
  private displayErrorMessage(message: string): void {
    console.log(message);
    // 可以在这里添加更多的错误处理逻辑，比如显示一个错误提示框
  }

  /**
   * 从一言API获取数据
   */
  private async fetchHitokoto(): Promise<void> {
    try {
      const response = await fetch('https://hitokoto.152710.xyz/');
      if (!response.ok) {
        throw new Error(`HTTP error! status: ${response.status}`);
      }
      const hitokotoData = await response.json() as Hitokoto;
      this.updateHitokotoDisplay(hitokotoData);
    } catch (error) {
      console.error('Failed to fetch hitokoto:', error);
      this.displayErrorMessage('获取一言数据失败');
    }
  }

  /**
   * 更新一言数据显示
   * @param hitokotoData 一言数据
   */
  private updateHitokotoDisplay(hitokotoData: Hitokoto): void {
    // 更新一言内容
    if (this.hitokotoContentElement && hitokotoData.hitokoto) {
      this.hitokotoContentElement.textContent = hitokotoData.hitokoto;
    }

    // 更新来源
    if (this.hitokotoFromElement && hitokotoData.from) {
      this.hitokotoFromElement.textContent = hitokotoData.from;
    }

    // 更新作者
    if (this.hitokotoAuthorElement && hitokotoData.from_who) {
      this.hitokotoAuthorElement.textContent = hitokotoData.from_who;
    }
  }

  /**
   * 从 /api/system-info 接口获取系统信息
   */
  private async fetchSystemInfo(): Promise<void> {
    try {
      const response = await fetch('/api/system-info');
      if (!response.ok) {
        throw new Error(`HTTP error! status: ${response.status}`);
      }
      const systemInfo = await response.json() as SystemInfo;
      this.updateSystemInfoDisplay(systemInfo);
    } catch (error) {
      console.error('Failed to fetch system info:', error);
      // 如果接口调用失败，使用模拟数据
      const mockSystemInfo = this.getMockSystemInfo();
      this.updateSystemInfoDisplay(mockSystemInfo);
    }
  }

  /**
   * 获取模拟系统信息
   * @returns 模拟的系统信息对象
   */
  private getMockSystemInfo(): SystemInfo {
    // 生成模拟的系统信息数据
    const usedMemory = Math.floor(Math.random() * 1000) + 2000;
    const totalMemory = 8192; // 模拟8GB内存

    const cpuUsage = Math.floor(Math.random() * 30) + 10;

    const usedDisk = Math.floor(Math.random() * 200) + 300;
    const totalDisk = 1000; // 模拟1TB磁盘

    const uptime = Math.floor(Math.random() * 86400) + 3600; // 模拟1小时到1天的运行时间

    return {
      memory: {
        total: totalMemory,
        used: usedMemory,
      },
      cpu: {
        usage: cpuUsage
      },
      disk: {
        total: totalDisk,
        used: usedDisk,
      },
      uptime: uptime,
      os: 'Windows 11',
      python_version: '3.11.4',
      project_memory: 1024,
      qq_memory: 2048,
    };
  }

  /**
   * 格式化运行时间
   * @param seconds 秒数
   * @returns 格式化的时间字符串
   */
  private formatUptime(seconds: number): string {
    const days = Math.floor(seconds / 86400);
    const hours = Math.floor((seconds % 86400) / 3600);
    const minutes = Math.floor((seconds % 3600) / 60);

    if (days > 0) {
      return `${days}天${hours}小时${minutes}分钟`;
    } else if (hours > 0) {
      return `${hours}小时${minutes}分钟`;
    } else {
      return `${minutes}分钟`;
    }
  }

  /**
   * 从 /api/plugins/status 接口获取插件信息
   */
  private async fetchPluginInfo(): Promise<void> {
    try {
      const response = await fetch('/api/plugins/status');
      if (!response.ok) {
        throw new Error(`HTTP error! status: ${response.status}`);
      }
      const pluginsData = await response.json() as PluginInfoResponse;
      this.updatePluginInfoDisplay(pluginsData);
    } catch (error) {
      console.error('Failed to fetch plugin info:', error);
      // 使用模拟数据
      this.updatePluginInfoDisplay({
        code: 0,
        data: {
          plugins_count: 1,
          enabled_count: 1,
          plugins: {}
        }
      });
    }
  }

  /**
   * 更新页面上的插件信息显示
   * @param pluginsData 插件数据
   */
  private updatePluginInfoDisplay(pluginsData: PluginInfoResponse): void {
    if (!pluginsData || !pluginsData.data) return;

    // 更新插件总数
    if (this.pluginsCountElement) {
      this.pluginsCountElement.textContent = `${pluginsData.data.plugins_count}`;
    }

    // 更新启用插件数量
    if (this.enabledPluginsCountElement) {
      this.enabledPluginsCountElement.textContent = `${pluginsData.data.enabled_count}`;
    }
  }

  /**
   * 重载所有插件
   */
  private async reloadPlugins(): Promise<void> {
    if (!this.reloadPluginsButton) return;

    // 禁用按钮并显示加载状态
    const originalText = this.reloadPluginsButton.textContent;
    this.reloadPluginsButton.disabled = true;
    this.reloadPluginsButton.textContent = '重载中...';

    try {
      // 调用重载插件的API路由
      const response = await fetch('/api/plugins/reload-all', {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json'
        }
      });

      if (!response.ok) {
        throw new Error(`HTTP error! status: ${response.status}`);
      }

      const result = await response.json();

      if (result.code === 0) {
        showAlert('成功', '插件重载成功！', 'success', true, 3000);
        // 重载成功后重新获取插件信息
        this.fetchPluginInfo();
      } else {
        throw new Error(result.message || '插件重载失败');
      }
    } catch (error) {
      console.error('Failed to reload plugins:', error);
      showAlert('错误', `插件重载失败：${error instanceof Error ? error.message : String(error)}`, 'error');
    } finally {
      // 恢复按钮状态
      this.reloadPluginsButton.disabled = false;
      this.reloadPluginsButton.textContent = originalText;
    }
  }

  /**
   * 更新系统信息显示
   * @param systemInfo 系统信息数据
   */
  private updateSystemInfoDisplay(systemInfo: SystemInfo): void {
    // 计算百分比值
    const memoryUsagePercent = Math.round(systemInfo.memory.used * 100 / systemInfo.memory.total * 10) / 10;
    const projectMemoryPercent = Math.round((systemInfo.project_memory + systemInfo.qq_memory) * 100 / systemInfo.memory.total * 10) / 10;
    const diskUsagePercent = Math.round(systemInfo.disk.used * 100 / systemInfo.disk.total * 10) / 10;

    // 格式化内存和磁盘大小为GB
    const usedMemoryGB = Math.round(systemInfo.memory.used / Math.pow(1024, 3) * 10) / 10;
    const totalMemoryGB = Math.round(systemInfo.memory.total / Math.pow(1024, 3) * 10) / 10;
    const projectMemoryMB = Math.round((systemInfo.project_memory + systemInfo.qq_memory) / Math.pow(1024, 2) * 10) / 10;
    const usedDiskGB = Math.round(systemInfo.disk.used / Math.pow(1024, 3) * 10) / 10;
    const totalDiskGB = Math.round(systemInfo.disk.total / Math.pow(1024, 3) * 10) / 10;

    // 更新CPU使用率
    if (this.cpuUsageElement) {
      this.cpuUsageElement.textContent = `${systemInfo.cpu.usage}%`;
    }
    if (this.cpuUsageBarElement) {
      this.cpuUsageBarElement.style.width = `${systemInfo.cpu.usage}%`;
    }

    // 更新内存使用信息
    if (this.memoryUsageElement) {
      this.memoryUsageElement.innerHTML = `${usedMemoryGB}GB / ${totalMemoryGB}GB (${memoryUsagePercent}%)<br/>AmiaBot 占用 ${projectMemoryMB}MB`;
    }
    if (this.memoryUsageBarElement) {
      this.memoryUsageBarElement.style.width = `${memoryUsagePercent}%`;
    }
    if (this.memoryUsageBarProjectElement) {
      this.memoryUsageBarProjectElement.style.width = `${projectMemoryPercent}%`;
    }

    // 更新磁盘空间信息
    if (this.diskUsageElement) {
      this.diskUsageElement.textContent = `${usedDiskGB}GB / ${totalDiskGB}GB (${diskUsagePercent}%)`;
    }
    if (this.diskUsageBarElement) {
      this.diskUsageBarElement.style.width = `${diskUsagePercent}%`;
    }

    // 更新运行时间
    if (this.uptimeElement) {
      this.uptimeElement.textContent = this.formatUptime(systemInfo.uptime);
    }

    // 更新操作系统信息
    if (this.osElement) {
      this.osElement.textContent = systemInfo.os;
    }

    // 更新Python版本信息
    if (this.pythonVersionElement) {
      this.pythonVersionElement.textContent = systemInfo.python_version;
    }
  }
}

/**
 * 用户信息接口类型定义
 */
interface UserInfo {
  qq: number;
  nick: string;
  remark: string;
  country: string;
  city: string;
  reg_time: number;
  qid: string;
  birthday: number;
  age: number;
  sex: 'male' | 'female' | 'unknown';
  avatar: string;
}

/**
 * 一言数据接口类型定义
 */
interface Hitokoto {
  id: number;
  uuid: string;
  hitokoto: string;
  type: string;
  from: string;
  from_who: string | null;
  creator: string;
  creator_uid: number;
  reviewer: number;
  commit_from: string;
  created_at: string;
  length: number;
}

// 页面加载完成后初始化用户信息管理器
if (document.readyState === 'loading') {
  document.addEventListener('DOMContentLoaded', () => {
    new UserInfoManager();
  });
} else {
  // 如果DOM已经加载完成，则直接初始化
  new UserInfoManager();
}

/**
 * 系统信息接口类型定义
 */
interface SystemInfo {
  memory: {
    total: number;
    used: number;
  };
  cpu: {
    usage: number;
  };
  disk: {
    total: number;
    used: number;
  };
  uptime: number;
  os: string;
  python_version: string;
  project_memory: number;
  qq_memory: number;
}
