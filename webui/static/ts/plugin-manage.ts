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

        // 设置按钮 - 新增的设置按钮
        const settingsButton = document.createElement('button');
        settingsButton.className = 'action-button settings-button';
        settingsButton.textContent = '设置';
        settingsButton.addEventListener('click', () => this.openPluginSettings(plugin.id));

        actionsCell.appendChild(toggleButton);
        actionsCell.appendChild(reloadButton);
        actionsCell.appendChild(settingsButton);

        // 组装插件行
        row.appendChild(nameCell);
        row.appendChild(idCell);
        row.appendChild(descriptionCell);
        row.appendChild(statusCell);
        row.appendChild(actionsCell);

        return row;
    }

    /**
     * 打开插件设置弹窗
     * @param pluginId 插件ID
     */
    private async openPluginSettings(pluginId: string): Promise<void> {
        try {
            // 获取插件配置
            const response = await fetch(`/api/plugin-config/get?id=${encodeURIComponent(pluginId)}`);
            if (!response.ok) {
                throw new Error(`HTTP error! status: ${response.status}`);
            }
            const result = await response.json();

            if (result.code === 0 && result.data) {
                const config = result.data;

                // 创建配置编辑表单
                const configForm = this.createConfigForm(pluginId, config);

                // 显示设置弹窗
                const alertElement = showAlert(
                    '插件设置',
                    configForm,
                    'info',
                    false,
                    0
                );

                // 添加保存按钮点击事件监听
                const saveButton = alertElement.querySelector('.save-config-button');
                if (saveButton) {
                    saveButton.addEventListener('click', () => this.savePluginConfig(pluginId, alertElement));
                }
            } else {
                throw new Error(result.message || '获取插件配置失败');
            }
        } catch (error) {
            console.error(`Failed to get plugin config for ${pluginId}:`, error);
            showAlert('错误', `获取插件配置失败：${error instanceof Error ? error.message : String(error)}`, 'error');
        }
    }

    /**
     * 创建配置编辑表单
     * @param pluginId 插件ID
     * @param config 当前配置对象
     * @returns 表单HTML元素
     */
    private createConfigForm(pluginId: string, config: any): HTMLElement {
        const formContainer = document.createElement('div');
        formContainer.className = 'config-form-container';

        // 插件ID显示
        const pluginIdElement = document.createElement('div');
        pluginIdElement.className = 'plugin-id-display';
        pluginIdElement.textContent = `插件ID: ${pluginId}`;
        formContainer.appendChild(pluginIdElement);

        // 如果配置为空，初始化默认结构
        if (!config.triggers) {
            config = { triggers: {} };
        }

        // 创建触发器配置区域
        const triggersContainer = document.createElement('div');
        triggersContainer.className = 'triggers-container';
        triggersContainer.innerHTML = '<h4>触发器配置</h4>';

        // 如果没有触发器配置，显示提示
        if (Object.keys(config.triggers).length === 0) {
            const emptyState = document.createElement('div');
            emptyState.className = 'empty-state';
            emptyState.textContent = '该插件暂无触发器配置';
            triggersContainer.appendChild(emptyState);
        } else {
            // 为每个触发器创建配置项
            Object.entries(config.triggers).forEach(([triggerId, triggerConfig]: [string, any]) => {
                const triggerSection = document.createElement('div');
                triggerSection.className = 'trigger-section';
                // 添加数据属性以便JS操作
                triggerSection.setAttribute('data-trigger-id', triggerId);

                // 触发器标题
                const triggerTitle = document.createElement('h5');
                triggerTitle.textContent = `触发器: ${triggerId}`;
                triggerSection.appendChild(triggerTitle);

                // 启用状态
                const enabledContainer = document.createElement('div');
                enabledContainer.className = 'config-item';
                enabledContainer.innerHTML = `
                    <label>
                        <input type="checkbox" name="${triggerId}_enabled" class="trigger-enabled" 
                               data-trigger-id="${triggerId}" ${triggerConfig.enabled ? 'checked' : ''}>
                        启用触发器
                    </label>
                `;
                triggerSection.appendChild(enabledContainer);

                // 私聊支持
                const privateContainer = document.createElement('div');
                privateContainer.className = 'config-item';
                privateContainer.innerHTML = `
                    <label>
                        <input type="checkbox" name="${triggerId}_can_private" class="trigger-can-private" 
                               data-trigger-id="${triggerId}" ${triggerConfig.can_private ? 'checked' : ''}>
                        允许私聊使用
                    </label>
                `;
                triggerSection.appendChild(privateContainer);

                // 群组配置
                const groupsContainer = document.createElement('div');
                groupsContainer.className = 'config-item';
                groupsContainer.innerHTML = `
                    <label>适用群组ID (用逗号分隔):</label>
                    <input type="text" name="${triggerId}_groups" class="trigger-groups" 
                           data-trigger-id="${triggerId}" value="${Array.isArray(triggerConfig.groups) ? triggerConfig.groups.join(', ') : ''}">
                    <small>留空表示适用于所有群组</small>
                `;
                triggerSection.appendChild(groupsContainer);

                triggersContainer.appendChild(triggerSection);
            });
        }

        formContainer.appendChild(triggersContainer);

        // 创建保存按钮
        const buttonContainer = document.createElement('div');
        buttonContainer.className = 'button-container';

        const saveButton = document.createElement('button');
        saveButton.className = 'save-config-button';
        saveButton.textContent = '保存配置';
        saveButton.setAttribute('data-action', 'save');
        // 添加微交互效果的辅助类
        saveButton.setAttribute('data-button-type', 'primary');

        buttonContainer.appendChild(saveButton);
        formContainer.appendChild(buttonContainer);

        return formContainer;
    }

    /**
     * 保存插件配置
     * @param pluginId 插件ID
     * @param alertElement 弹窗元素
     */
    private async savePluginConfig(pluginId: string, alertElement: HTMLElement): Promise<void> {
        try {
            // 获取保存按钮并显示加载状态
            const saveButton = alertElement.querySelector('.save-config-button[data-action="save"]') as HTMLButtonElement;
            if (saveButton) {
                // 保存原始文本并禁用按钮
                const originalText = saveButton.textContent;
                saveButton.textContent = '保存中...';
                saveButton.disabled = true;

                try {
                    // 构建新的配置对象
                    const newConfig: any = { triggers: {} };

                    // 获取所有触发器配置
                    const triggerSections = alertElement.querySelectorAll('.trigger-section');
                    triggerSections.forEach(section => {
                        // 使用数据属性获取触发器ID，提高代码健壮性
                        const triggerId = section.getAttribute('data-trigger-id') || '';

                        if (triggerId) {
                            // 获取配置值
                            const enabledCheckbox = section.querySelector(`.trigger-enabled[data-trigger-id="${triggerId}"]`) as HTMLInputElement;
                            const privateCheckbox = section.querySelector(`.trigger-can-private[data-trigger-id="${triggerId}"]`) as HTMLInputElement;
                            const groupsInput = section.querySelector(`.trigger-groups[data-trigger-id="${triggerId}"]`) as HTMLInputElement;

                            // 解析群组ID
                            let groups: string[] = [];
                            if (groupsInput && groupsInput.value.trim()) {
                                groups = groupsInput.value.split(',').map(group => group.trim()).filter(Boolean);
                            }

                            // 添加到配置对象
                            newConfig.triggers[triggerId] = {
                                enabled: enabledCheckbox ? enabledCheckbox.checked : false,
                                can_private: privateCheckbox ? privateCheckbox.checked : false,
                                groups: groups
                            };
                        }
                    });

                    // 发送保存请求
                    const response = await fetch('/api/plugin-config/set', {
                        method: 'POST',
                        headers: {
                            'Content-Type': 'application/json'
                        },
                        body: JSON.stringify({
                            plugin_id: pluginId,
                            config: newConfig
                        })
                    });

                    if (!response.ok) {
                        throw new Error(`HTTP错误! 状态码: ${response.status}`);
                    }

                    const result = await response.json();

                    if (result.code === 0) {
                        showAlert('成功', `插件配置保存成功！`, 'success', true, 3000);
                        // 关闭设置弹窗
                        removeAlert(alertElement);
                    } else {
                        throw new Error(result.message || '保存插件配置失败');
                    }
                } catch (error) {
                    console.error(`保存插件配置失败 (${pluginId}):`, error);
                    // 恢复按钮状态
                    saveButton.textContent = originalText;
                    saveButton.disabled = false;
                    // 显示错误提示
                    showAlert('错误', `保存插件配置失败：${error instanceof Error ? error.message : String(error)}`, 'error');
                }
            }
        } catch (error) {
            console.error(`保存插件配置时发生异常 (${pluginId}):`, error);
            showAlert('错误', `保存插件配置时发生异常：${error instanceof Error ? error.message : String(error)}`, 'error');
        }
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
                showAlert('成功', '所有插件重载成功！', 'success', true, 3000);
                // 重载成功后重新获取插件信息
                this.fetchPluginsInfo();
            } else {
                throw new Error(result.message || '所有插件重载失败');
            }
        } catch (error) {
            console.error('Failed to reload all plugins:', error);
            showAlert('错误', `所有插件重载失败：${error instanceof Error ? error.message : String(error)}`, 'error');
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
                showAlert('成功', `插件 ${pluginId} 重载成功！`, 'success', true, 3000);
                // 重载成功后重新获取插件信息
                this.fetchPluginsInfo();
            } else {
                throw new Error(result.message || `插件 ${pluginId} 重载失败`);
            }
        } catch (error) {
            console.error(`Failed to reload plugin ${pluginId}:`, error);
            showAlert('错误', `插件 ${pluginId} 重载失败：${error instanceof Error ? error.message : String(error)}`, 'error');
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
                showAlert('成功', `插件 ${pluginId} 启用成功！`, 'success', true, 3000);
                // 启用成功后重新获取插件信息
                this.fetchPluginsInfo();
            } else {
                throw new Error(result.message || `插件 ${pluginId} 启用失败`);
            }
        } catch (error) {
            console.error(`Failed to enable plugin ${pluginId}:`, error);
            showAlert('错误', `插件 ${pluginId} 启用失败：${error instanceof Error ? error.message : String(error)}`, 'error');
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
                showAlert('成功', `插件 ${pluginId} 禁用成功！`, 'success', true, 3000);
                // 禁用成功后重新获取插件信息
                this.fetchPluginsInfo();
            } else {
                throw new Error(result.message || `插件 ${pluginId} 禁用失败`);
            }
        } catch (error) {
            console.error(`Failed to disable plugin ${pluginId}:`, error);
            showAlert('错误', `插件 ${pluginId} 禁用失败：${error instanceof Error ? error.message : String(error)}`, 'error');
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
