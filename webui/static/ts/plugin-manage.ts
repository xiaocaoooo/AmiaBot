/**
 * 插件管理类
 * 负责从 /api/plugins/status 接口获取插件信息并更新页面显示
 * 处理插件的重载、启用和禁用等操作
 */
class PluginManager {
    // 插件数据
    private plugins: Record<string, PluginInfo> = {};

    // 页面元素引用
    private totalPluginsElement: HTMLElement | null = null;
    private enabledPluginsElement: HTMLElement | null = null;
    private disabledPluginsElement: HTMLElement | null = null;
    private pluginsListElement: HTMLElement | null = null;
    private refreshPluginsButton: HTMLButtonElement | null = null;
    private reloadAllPluginsButton: HTMLButtonElement | null = null;

    /**
     * 构造函数，初始化插件管理器
     */
    constructor() {
        // 获取页面元素引用
        this.totalPluginsElement = document.getElementById('total-plugins-count');
        this.enabledPluginsElement = document.getElementById('enabled-plugins-count');
        this.disabledPluginsElement = document.getElementById('disabled-plugins-count');
        this.pluginsListElement = document.getElementById('plugins-container');
        this.refreshPluginsButton = document.getElementById('refresh-plugins-button') as HTMLButtonElement;
        this.reloadAllPluginsButton = document.getElementById('reload-all-plugins-button') as HTMLButtonElement;

        // 初始化事件监听器
        this.initEventListeners();

        // 初始加载插件信息
        this.fetchPluginsInfo();

        // 设置定时器定期更新插件信息
        // setInterval(() => this.fetchPluginsInfo(), 5000); // 每5秒更新一次 - 已取消自动刷新
    }

    /**
     * 初始化事件监听器
     */
    private initEventListeners(): void {
        // 刷新插件列表按钮点击事件
        if (this.refreshPluginsButton) {
            this.refreshPluginsButton.addEventListener('click', () => this.fetchPluginsInfo());
        }

        // 重载所有插件按钮点击事件
        if (this.reloadAllPluginsButton) {
            this.reloadAllPluginsButton.addEventListener('click', () => this.reloadAllPlugins());
        }
    }

    /**
     * 从 /api/plugins/status 接口获取插件信息
     */
    private async fetchPluginsInfo(): Promise<void> {
        try {
            const response = await fetch('/api/plugins/status');
            if (!response.ok) {
                throw new Error(`HTTP error! status: ${response.status}`);
            }
            const pluginsData = await response.json() as PluginInfoResponse;
            console.log('pluginsData', pluginsData);
            this.updatePluginsDisplay(pluginsData);
        } catch (error) {
            console.error('Failed to fetch plugins info:', error);
            this.displayErrorMessage('获取插件信息失败');

            // 使用模拟数据避免页面空白 - 使用类型断言解决类型冲突
            const mockData = {
                code: 0,
                data: {
                    plugins_count: 2,
                    enabled_count: 2,
                    plugins: {
                        'plugin_id_1': {
                            id: 'plugin_id_1',
                            name: '插件名称 1',
                            description: '描述 1',
                            enabled: true,
                            loaded: true,
                            file_name: 'plugin_1',
                            file_path: './plugin_1'
                        } as PluginInfo,
                        'plugin_id_2': {
                            id: 'plugin_id_2',
                            name: '插件名称 2',
                            description: '描述 2',
                            enabled: true,
                            loaded: true,
                            file_name: 'plugin_2',
                            file_path: './plugin_2'
                        } as PluginInfo
                    }
                }
            };
            this.updatePluginsDisplay(mockData as PluginInfoResponse);
        }
    }

    /**
     * 更新页面上的插件信息显示
     * @param pluginsData 插件数据
     */
    private updatePluginsDisplay(pluginsData: PluginInfoResponse): void {
        if (!pluginsData || !pluginsData.data) return;

        // 更新插件数据
        this.plugins = pluginsData.data.plugins;

        // 更新统计信息
        const totalCount = pluginsData.data.plugins_count;
        const enabledCount = pluginsData.data.enabled_count;
        const disabledCount = totalCount - enabledCount;

        if (this.totalPluginsElement) {
            this.totalPluginsElement.textContent = `${totalCount}`;
        }

        if (this.enabledPluginsElement) {
            this.enabledPluginsElement.textContent = `${enabledCount}`;
        }

        if (this.disabledPluginsElement) {
            this.disabledPluginsElement.textContent = `${disabledCount}`;
        }

        // 更新插件列表
        this.updatePluginsList();
    }

    /**
     * 更新插件列表显示
     */
    private updatePluginsList(): void {
        if (!this.pluginsListElement) return;
        this.pluginsListElement.innerHTML = '';

        // 保存当前列表中的静态数据（如果有）
        const staticRows = Array.from(this.pluginsListElement.querySelectorAll('.plugin'))
            .filter(row => !row.hasAttribute('data-dynamic'));

        // 清空插件列表
        this.pluginsListElement.innerHTML = '';

        // 如果有静态数据，先添加静态数据
        staticRows.forEach(row => {
            this.pluginsListElement?.appendChild(row);
        });

        // 如果没有插件，显示空状态
        if (Object.keys(this.plugins).length === 0) {
            const emptyRow = document.createElement('div');
            emptyRow.className = 'plugin empty-row';
            emptyRow.innerHTML = '<div style="width: 100%; text-align: center; padding: 20px;" class="plugin-name">暂无插件</div>';
            this.pluginsListElement.appendChild(emptyRow);
            return;
        }

        // 遍历插件数据，创建插件行
        Object.keys(this.plugins).forEach(pluginId => {
            const plugin = this.plugins[pluginId];
            const pluginRow = this.createPluginRow(plugin);
            if (this.pluginsListElement) {
                this.pluginsListElement.appendChild(pluginRow);
            }
        });
    }

    /**
     * 创建插件行元素
     * @param plugin 插件数据
     * @returns 创建的插件行元素
     */
    private createPluginRow(plugin: PluginInfo): HTMLElement {
        const row = document.createElement('div');
        row.className = 'plugin secondary-container';
        row.setAttribute('data-dynamic', 'true');

        // 插件名称单元格
        const nameCell = document.createElement('div');
        nameCell.className = 'plugin-name';
        nameCell.textContent = plugin.name || plugin.id;

        // 插件ID单元格
        const idCell = document.createElement('div');
        idCell.className = 'plugin-id';
        idCell.textContent = plugin.id;

        // 插件描述单元格
        const descriptionCell = document.createElement('div');
        descriptionCell.className = 'plugin-description';
        descriptionCell.textContent = plugin.description || '无描述';

        // 插件状态单元格
        const statusCell = document.createElement('div');
        statusCell.className = 'plugin-status';

        const statusBadge = document.createElement('span');
        statusBadge.className = `status-badge ${plugin.enabled ? 'enabled' : 'disabled'}`;
        statusBadge.textContent = plugin.enabled ? '已启用' : '已禁用';
        statusCell.appendChild(statusBadge);

        // 操作按钮单元格
        const actionsCell = document.createElement('div');
        actionsCell.className = 'plugin-actions';

        // 切换状态按钮
        const toggleButton = document.createElement('button');
        toggleButton.className = `action-button toggle-button ${plugin.enabled ? 'disable' : 'enable'}`;
        toggleButton.textContent = plugin.enabled ? '禁用' : '启用';
        toggleButton.addEventListener('click', () => {
            if (plugin.enabled) {
                this.disablePlugin(plugin.id);
            } else {
                this.enablePlugin(plugin.id);
            }
        });

        // 重载按钮
        const reloadButton = document.createElement('button');
        reloadButton.className = 'action-button reload-button';
        reloadButton.textContent = '重载';
        reloadButton.addEventListener('click', () => this.reloadPlugin(plugin.id));

        actionsCell.appendChild(toggleButton);
        actionsCell.appendChild(reloadButton);

        // 组装插件行
        row.appendChild(nameCell);
        row.appendChild(idCell);
        row.appendChild(descriptionCell);
        row.appendChild(statusCell);
        row.appendChild(actionsCell);

        return row;
    }

    /**
     * 重载所有插件
     */
    private async reloadAllPlugins(): Promise<void> {
        if (!this.reloadAllPluginsButton) return;

        // 禁用按钮并显示加载状态
        const originalText = this.reloadAllPluginsButton.textContent;
        this.reloadAllPluginsButton.disabled = true;
        this.reloadAllPluginsButton.textContent = '重载中...';

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
                alert('所有插件重载成功！');
                // 重载成功后重新获取插件信息
                this.fetchPluginsInfo();
            } else {
                throw new Error(result.message || '所有插件重载失败');
            }
        } catch (error) {
            console.error('Failed to reload all plugins:', error);
            alert(`所有插件重载失败：${error instanceof Error ? error.message : String(error)}`);
        } finally {
            // 恢复按钮状态
            this.reloadAllPluginsButton.disabled = false;
            this.reloadAllPluginsButton.textContent = originalText;
        }
    }

    /**
     * 重载单个插件
     * @param pluginId 插件ID
     */
    private async reloadPlugin(pluginId: string): Promise<void> {
        try {
            // 调用重载单个插件的API路由
            const response = await fetch('/api/plugins/reload', {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json'
                },
                body: JSON.stringify({ plugin_id: pluginId })
            });

            if (!response.ok) {
                throw new Error(`HTTP error! status: ${response.status}`);
            }

            const result = await response.json();

            if (result.code === 0) {
                alert(`插件 ${pluginId} 重载成功！`);
                // 重载成功后重新获取插件信息
                this.fetchPluginsInfo();
            } else {
                throw new Error(result.message || `插件 ${pluginId} 重载失败`);
            }
        } catch (error) {
            console.error(`Failed to reload plugin ${pluginId}:`, error);
            alert(`插件 ${pluginId} 重载失败：${error instanceof Error ? error.message : String(error)}`);
        }
    }

    /**
     * 启用插件
     * @param pluginId 插件ID
     */
    private async enablePlugin(pluginId: string): Promise<void> {
        try {
            // 调用启用插件的API路由
            const response = await fetch('/api/plugins/enable', {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json'
                },
                body: JSON.stringify({ plugin_id: pluginId })
            });

            if (!response.ok) {
                throw new Error(`HTTP error! status: ${response.status}`);
            }

            const result = await response.json();

            if (result.code === 0) {
                alert(`插件 ${pluginId} 启用成功！`);
                // 启用成功后重新获取插件信息
                this.fetchPluginsInfo();
            } else {
                throw new Error(result.message || `插件 ${pluginId} 启用失败`);
            }
        } catch (error) {
            console.error(`Failed to enable plugin ${pluginId}:`, error);
            alert(`插件 ${pluginId} 启用失败：${error instanceof Error ? error.message : String(error)}`);
        }
    }

    /**
     * 禁用插件
     * @param pluginId 插件ID
     */
    private async disablePlugin(pluginId: string): Promise<void> {
        try {
            // 调用禁用插件的API路由
            const response = await fetch('/api/plugins/disable', {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json'
                },
                body: JSON.stringify({ plugin_id: pluginId })
            });

            if (!response.ok) {
                throw new Error(`HTTP error! status: ${response.status}`);
            }

            const result = await response.json();

            if (result.code === 0) {
                alert(`插件 ${pluginId} 禁用成功！`);
                // 禁用成功后重新获取插件信息
                this.fetchPluginsInfo();
            } else {
                throw new Error(result.message || `插件 ${pluginId} 禁用失败`);
            }
        } catch (error) {
            console.error(`Failed to disable plugin ${pluginId}:`, error);
            alert(`插件 ${pluginId} 禁用失败：${error instanceof Error ? error.message : String(error)}`);
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
}

/**
 * 插件接口类型定义
 */
interface PluginInfo {
    id: string;
    name: string;
    description: string;
    enabled: boolean;
    loaded: boolean;
    file_name: string;
    file_path: string;
}

/**
 * 插件信息响应接口类型定义
 */
interface PluginInfoResponse {
    code: number;
    data: {
        plugins_count: number;
        enabled_count: number;
        plugins: Record<string, PluginInfo>;
    };
    message?: string;
}

// 页面加载完成后初始化插件管理器
if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', () => {
        new PluginManager();
    });
} else {
    // 如果DOM已经加载完成，则直接初始化
    new PluginManager();
}
