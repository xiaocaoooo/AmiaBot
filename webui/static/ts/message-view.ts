/**
 * 消息数据接口定义
 */
interface MessageEvent {
  self_id: number;
  user_id: number;
  time: number;
  message_id: number;
  message_seq: number;
  real_id: number;
  real_seq: string;
  message_type: string;
  sender: {
    user_id: number;
    nickname: string;
    card: string;
    role?: string;
  };
  raw_message: string;
  font: number;
  sub_type: string;
  message: Array<{
    type: string;
    data: any;
  }>;
  message_format: string;
  post_type: string;
  group_id?: number;
  group_name?: string;
}

/**
 * 日志条目接口，用于包装消息事件
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
 * 消息数据响应接口定义
 */
interface MessagesResponse {
  code: number;
  data: {
    logs: LogEntry[];
    pagination: PaginationInfo;
  };
  message?: string;
}

/**
 * 消息查看器类
 * 负责从 /api/logs 接口获取消息数据并显示在页面上
 */
class MessageViewer {
  // 当前页码
  private currentPage: number = 1;
  // 每页显示的消息数量
  private pageSize: number = 20;
  // 消息数据 - 支持存储任意类型的对象
  private messagesData: any[] = [];
  // 分页信息
  private paginationInfo: PaginationInfo | null = null;

  // 页面元素引用
  private messagesContainer: HTMLElement | null = null;
  private refreshMessagesButton: HTMLButtonElement | null = null;
  private toggleAutoRefreshButton: HTMLButtonElement | null = null;
  private totalMessagesCountElement: HTMLElement | null = null;
  private currentPageElement: HTMLElement | null = null;
  private totalPagesElement: HTMLElement | null = null;
  private pageInfoElement: HTMLElement | null = null;
  private prevPageButton: HTMLButtonElement | null = null;
  private nextPageButton: HTMLButtonElement | null = null;

  // 自动刷新相关属性
  private autoRefreshInterval: number = 1000; // 用户要求1秒刷新一次
  private autoRefreshTimer: number | null = null; // 自动刷新定时器ID
  private isAutoRefreshEnabled: boolean = true; // 是否启用自动刷新

  /**
   * 构造函数，初始化消息查看器
   */
  constructor() {
    // 获取页面元素引用
    this.messagesContainer = document.getElementById('messages-container');
    this.refreshMessagesButton = document.getElementById('refresh-messages-button') as HTMLButtonElement;
    this.toggleAutoRefreshButton = document.getElementById('toggle-auto-refresh-button') as HTMLButtonElement;
    this.totalMessagesCountElement = document.getElementById('total-messages-count');
    this.currentPageElement = document.getElementById('current-page');
    this.totalPagesElement = document.getElementById('total-pages');
    this.pageInfoElement = document.getElementById('page-info');
    this.prevPageButton = document.getElementById('prev-page') as HTMLButtonElement;
    this.nextPageButton = document.getElementById('next-page') as HTMLButtonElement;

    // 设置事件监听器
    this.initEventListeners();

    // 初始加载消息数据
    this.fetchMessages();

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
    if (this.refreshMessagesButton) {
      this.refreshMessagesButton.addEventListener('click', () => this.refreshMessages());
    }

    // 自动刷新切换按钮点击事件
    if (this.toggleAutoRefreshButton) {
      this.toggleAutoRefreshButton.addEventListener('click', () => this.toggleAutoRefresh());
    }

    // 上一页按钮点击事件
    if (this.prevPageButton) {
      this.prevPageButton.addEventListener('click', () => this.goToPrevPage());
    }

    // 下一页按钮点击事件
    if (this.nextPageButton) {
      this.nextPageButton.addEventListener('click', () => this.goToNextPage());
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
      this.fetchMessages();
    }, this.autoRefreshInterval);
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
    if (this.toggleAutoRefreshButton) {
      if (this.isAutoRefreshEnabled) {
        this.toggleAutoRefreshButton.classList.remove('active');
        this.toggleAutoRefreshButton.textContent = '开启自动刷新';
      } else {
        this.toggleAutoRefreshButton.classList.add('active');
        this.toggleAutoRefreshButton.textContent = '关闭自动刷新';
      }
    }
  }

  /**
   * 从 /api/logs 接口获取消息数据
   */
  private async fetchMessages(): Promise<void> {
    try {
      // 构建请求URL，使用only_message=true参数
      const url = `/api/logs?page=${this.currentPage}&page_size=${this.pageSize}&only_message=true`;
      const response = await fetch(url);

      if (!response.ok) {
        throw new Error(`HTTP error! status: ${response.status}`);
      }

      const messagesResponse = await response.json() as MessagesResponse;

      if (messagesResponse.code === 0 && messagesResponse.data) {
        // 解析消息事件
        this.messagesData = this.parseMessageEvents(messagesResponse.data.logs);
        this.paginationInfo = messagesResponse.data.pagination;
        this.updateMessagesDisplay();
      } else {
        throw new Error(messagesResponse.message || 'Failed to fetch messages');
      }
    } catch (error) {
      console.error('Error fetching messages:', error);
      // this.showErrorMessage('获取消息数据失败');
    }
  }

  /**
   * 解析日志中的消息事件
   */
  private parseMessageEvents(logs: LogEntry[]): any[] {
    const messageEvents: any[] = [];

    logs.forEach(log => {
      try {
        // 提取消息JSON部分
        if (log.message && log.message.startsWith('Received message: ')) {
          const jsonPart = log.message.substring('Received message: '.length);
          // 直接解析成通用对象并存储，不做类型限制
          const rawData = JSON.parse(jsonPart);
          messageEvents.push(rawData);
        }
      } catch (error) {
        console.error('Error parsing message event:', error);
      }
    });

    // 按照时间倒序排序，最新的在最上面
    return messageEvents.sort((a, b) => {
      // 尝试获取时间戳进行排序，如果没有则按默认顺序
      const timeA = a && a.time ? a.time : 0;
      const timeB = b && b.time ? b.time : 0;
      return timeB - timeA;
    });
  }

  /**
   * 更新消息显示
   */
  private updateMessagesDisplay(): void {
    // 更新消息列表
    this.updateMessagesList(this.messagesData);

    // 更新分页信息
    this.updatePaginationInfo();

    // 检查是否有空消息情况
    if (this.messagesData.length === 0) {
      this.showEmptyState();
    } else {
      this.hideEmptyState();
    }
  }

  /**
   * 更新消息列表
   */
  private updateMessagesList(messages: any[]): void {
    if (!this.messagesContainer) return;

    // 清空容器
    this.messagesContainer.innerHTML = '';

    // 添加消息项
    messages.forEach(message => {
      const messageItem = document.createElement('div');
      messageItem.className = 'message-item';

      // 格式化时间
      let formattedTime = '未知时间';
      if (message && message.time) {
        try {
          formattedTime = new Date(message.time * 1000).toLocaleString('zh-CN', {
            year: 'numeric',
            month: '2-digit',
            day: '2-digit',
            hour: '2-digit',
            minute: '2-digit',
            second: '2-digit'
          });
        } catch (e) {
          formattedTime = '时间解析失败';
        }
      }

      // 创建消息内容
      let messageContent = '';
      if (message && (message.message && message.message.length > 0 || message.raw_message)) {
        // 对于符合MessageEvent格式的消息，使用原有逻辑渲染
        if (message.post_type === 'message' || message.post_type === 'message_sent') {
          const messageEvent = message as MessageEvent;
          if (messageEvent.message && messageEvent.message.length > 0) {
            messageContent = this.renderMessageContent(messageEvent.message);
          } else {
            messageContent = messageEvent.raw_message || '';
          }
        } else if (message.post_type === 'notice') {
          // 对于通知事件，使用专门的渲染方法
          messageContent = this.renderNoticeEvent(message);
        } else if (message.post_type === 'meta_event') {
          // 对于元事件，使用专门的渲染方法
          messageContent = this.renderMetaEvent(message);
        } else {
          // 对于其他不符合格式的消息，直接展示原始数据
          try {
            const rawContent = JSON.stringify(message, null, 2);
            messageContent = `<pre class="raw-message-data">${rawContent}</pre>`;
          } catch (e) {
            messageContent = '[无法解析的消息数据]';
          }
        }
      } else if (message && message.post_type === 'notice') {
        // 对于没有message但post_type为notice的消息，使用专门的渲染方法
        messageContent = this.renderNoticeEvent(message);
      } else if (message && message.post_type === 'meta_event') {
        // 对于没有message但post_type为meta_event的消息，使用专门的渲染方法
        messageContent = this.renderMetaEvent(message);
      } else {
        // 尝试显示原始消息数据
        try {
          const rawContent = JSON.stringify(message, null, 2);
          messageContent = `<pre class="raw-message-data">${rawContent}</pre>`;
        } catch (e) {
          messageContent = '[无法解析的消息数据]';
        }
      }

      // 构建消息HTML结构
      let senderInfo = '';
      let metaInfo = '';
      let messageTypeInfo = '';
      
      if (message && (message.post_type === 'message' || message.post_type === 'message_sent')) {
        const messageEvent = message as MessageEvent;
        
        // 确定消息类型图标
        const messageTypeIcon = messageEvent.message_type === 'group' ? 'users' : 'user';
        const messageTypeLabel = messageEvent.message_type === 'group' ? '群消息' : '私聊消息';
        
        messageTypeInfo = `
          <div class="message-type">
            <i class="fas fa-${messageTypeIcon}"></i> ${messageTypeLabel}
          </div>
        `;
        
        // 发送者信息
        if (messageEvent.sender) {
          senderInfo = `
            <div class="message-sender">
              <span class="sender-nickname">${messageEvent.sender.nickname}</span>
              ${messageEvent.sender.card ? `<span class="sender-card">(${messageEvent.sender.card})</span>` : ''}
              ${messageEvent.group_name ? `<span class="group-name">[${messageEvent.group_name}]</span>` : ''}
            </div>
          `;
        }
        
        // 元信息
        metaInfo = `
          <div class="message-meta">
            ${messageEvent.message_id ? `<span class="message-id">消息ID: ${messageEvent.message_id}</span>` : ''}
            ${messageEvent.sender && messageEvent.sender.user_id ? `<span class="sender-id">发送者ID: ${messageEvent.sender.user_id}</span>` : ''}
          </div>
        `;
      } else if (message && message.post_type === 'notice') {
        // 对于通知事件，显示特定图标和类型
        const noticeIcon = this.getNoticeTypeIcon(message.notice_type);
        const noticeLabel = this.getNoticeTypeLabel(message.notice_type);
        
        messageTypeInfo = `
          <div class="message-type notice-type">
            <i class="fas fa-${noticeIcon}"></i> 通知 - ${noticeLabel}
          </div>
        `;
        
        // 通知事件的元信息
        const metaFields = [];
        if (message.user_id) metaFields.push(`用户ID: ${message.user_id}`);
        if (message.group_id) metaFields.push(`群ID: ${message.group_id}`);
        if (message.notice_type) metaFields.push(`通知类型: ${message.notice_type}`);
        if (message.sub_type) metaFields.push(`子类型: ${message.sub_type}`);
        
        if (metaFields.length > 0) {
          metaInfo = `
            <div class="message-meta">
              ${metaFields.join('<span class="meta-separator">|</span>')}
            </div>
          `;
        }
      } else if (message && message.post_type === 'meta_event') {
        // 对于元事件，显示特定图标和类型
        const metaIcon = this.getMetaEventTypeIcon(message.meta_event_type);
        const metaLabel = this.getMetaEventTypeLabel(message.meta_event_type);
        
        messageTypeInfo = `
          <div class="message-type meta-type">  
            <i class="fas fa-${metaIcon}"></i> 元事件 - ${metaLabel}
          </div>
        `;
        
        // 元事件的元信息
        const metaFields = [];
        if (message.meta_event_type) metaFields.push(`元事件类型: ${message.meta_event_type}`);
        if (message.self_id) metaFields.push(`Bot ID: ${message.self_id}`);
        
        if (metaFields.length > 0) {
          metaInfo = `
            <div class="message-meta">
              ${metaFields.join('<span class="meta-separator">|</span>')}
            </div>
          `;
        }
      } else {
        // 对于其他不符合格式的消息，显示post_type
        messageTypeInfo = `
          <div class="message-type">
            <i class="fas fa-code"></i> ${message && message.post_type ? message.post_type : '未知类型'}
          </div>
        `;
      }
      
      messageItem.innerHTML = `
          <div class="message-header">
            <div class="message-time">${formattedTime}</div>
            ${messageTypeInfo}
          </div>
          ${senderInfo}
          <div class="message-content">${messageContent}</div>
          ${metaInfo}
        `;

      // 添加到容器
      if (this.messagesContainer) {
        this.messagesContainer.appendChild(messageItem);
      }
    });
  }

  /**
   * 渲染消息内容
   */
  private renderMessageContent(messageElements: Array<{ type: string; data: any }>): string {
    // 解析消息内容，处理各种类型的消息元素
    let content = '';

    messageElements.forEach(element => {
      switch (element.type) {
        case 'text':
          content += element.data.text || '';
          break;
        case 'image':
          content += '<span class="message-image">[图片]</span>';
          break;
        case 'at':
          content += `<span class="message-at">@${element.data.qq}</span>`;
          break;
        case 'face':
          content += '<span class="message-face">[表情]</span>';
          break;
        case 'reply':
          content += `<span class="message-reply">[回复消息ID: ${element.data.id}]</span>`;
          break;
        case 'file':
          content += `<span class="message-file">[文件: ${element.data.name || element.data.file || '未知文件名'}]</span>`;
          break;
        case 'video':
          content += '<span class="message-video">[视频]</span>';
          break;
        case 'voice':
        case 'record':
          content += '<span class="message-voice">[语音]</span>';
          break;
        case 'music':
          content += this.renderMusicMessage(element.data);
          break;
        case 'mface':
          content += '<span class="message-mface">[商城表情]</span>';
          break;
        case 'markdown':
          content += '<span class="message-markdown">[Markdown消息]</span>';
          break;
        case 'forward':
          content += '<span class="message-forward">[合并转发]</span>';
          break;
        case 'xml':
          content += '<span class="message-xml">[XML消息]</span>';
          break;
        case 'poke':
          content += '<span class="message-poke">[戳一戳]</span>';
          break;
        case 'dice':
          content += `<span class="message-dice">[骰子: ${element.data.result || '?'}]</span>`;
          break;
        case 'rps':
          content += `<span class="message-rps">[石头剪刀布: ${this.getRpsResult(element.data.result)}]</span>`;
          break;
        case 'miniapp':
          content += '<span class="message-miniapp">[小程序]</span>';
          break;
        case 'contact':
          content += this.renderContactMessage(element.data);
          break;
        case 'location':
          content += '<span class="message-location">[位置]</span>';
          break;
        case 'json':
          content += '<span class="message-json">[JSON消息]</span>';
          break;
        case 'node':
          content += '<span class="message-node">[转发节点]</span>';
          break;
        default:
          // 直接显示不支持类型的数据内容
          try {
            const dataContent = JSON.stringify(element.data);
            // 限制显示长度，避免过长的内容
            const displayContent = dataContent.length > 100 ? dataContent.substring(0, 97) + '...' : dataContent;
            content += `<span class="message-unknown">[${element.type}: ${displayContent}]</span>`;
          } catch (e) {
            // 处理JSON序列化失败的情况
            content += `<span class="message-unknown">[${element.type}]</span>`;
          }
          break;
      }
    });

    return content || '[无内容]';
  }

  /**
   * 渲染音乐消息
   */
  private renderMusicMessage(data: any): string {
    if (data.title && data.singer) {
      return `<span class="message-music">[音乐: ${data.title} - ${data.singer}]</span>`;
    } else if (data.content) {
      return `<span class="message-music">[音乐: ${data.content}]</span>`;
    }
    return '<span class="message-music">[音乐]</span>';
  }

  /**
   * 渲染联系人消息
   */
  private renderContactMessage(data: any): string {
    const type = data.type === 'qq' ? '好友' : data.type === 'group' ? '群聊' : '联系人';
    return `<span class="message-contact">[${type}: ${data.id}]</span>`;
  }

  /**
   * 获取通知类型对应的图标
   */
  private getNoticeTypeIcon(noticeType: string): string {
    const iconMap: { [key: string]: string } = {
      'group_increase': 'user-plus',
      'group_decrease': 'user-minus',
      'group_admin': 'crown',
      'group_upload': 'file-upload',
      'group_ban': 'user-lock',
      'friend_add': 'user-friends',
      'group_recall': 'undo',
      'friend_recall': 'undo',
      'notify': 'hand-pointer'
    };
    return iconMap[noticeType] || 'bell';
  }

  /**
   * 获取通知类型对应的人类可读标签
   */
  private getNoticeTypeLabel(noticeType: string): string {
    const labelMap: { [key: string]: string } = {
      'group_increase': '群成员增加',
      'group_decrease': '群成员减少',
      'group_admin': '管理员变更',
      'group_upload': '文件上传',
      'group_ban': '禁言操作',
      'friend_add': '好友添加',
      'group_recall': '群消息撤回',
      'friend_recall': '好友消息撤回',
      'notify': '通知事件'
    };
    return labelMap[noticeType] || noticeType;
  }

  /**
   * 获取元事件类型对应的图标
   */
  private getMetaEventTypeIcon(metaEventType: string): string {
    const iconMap: { [key: string]: string } = {
      'heartbeat': 'heartbeat',
      'lifecycle': 'circle-notch'
    };
    return iconMap[metaEventType] || 'circle';
  }

  /**
   * 获取元事件类型对应的标签
   */
  private getMetaEventTypeLabel(metaEventType: string): string {
    const labelMap: { [key: string]: string } = {
      'heartbeat': '心跳',
      'lifecycle': '生命周期'
    };
    return labelMap[metaEventType] || metaEventType;
  }

  /**
   * 渲染元事件内容
   */
  private renderMetaEvent(metaEvent: any): string {
    if (!metaEvent) {
      return '[无效的元事件]';
    }

    try {
      // 根据meta_event_type处理不同类型的元事件
      if (metaEvent.meta_event_type === 'heartbeat') {
        // 处理心跳事件
        const status = metaEvent.status || {};
        const onlineStatus = status.online ? '在线' : '离线';
        const goodStatus = status.good ? '正常' : '异常';
        const interval = metaEvent.interval || 0;
        
        return `
          <div class="meta-event-content">
            <div class="meta-event-item">
              <span class="meta-event-label">状态:</span>
              <span class="meta-event-value online-status ${status.online ? 'status-online' : 'status-offline'}">${onlineStatus}</span>
            </div>
            <div class="meta-event-item">
              <span class="meta-event-label">运行状态:</span>
              <span class="meta-event-value good-status ${status.good ? 'status-good' : 'status-bad'}">${goodStatus}</span>
            </div>
            <div class="meta-event-item">
              <span class="meta-event-label">心跳间隔:</span>
              <span class="meta-event-value">${interval}ms</span>
            </div>
          </div>
        `;
      } else if (metaEvent.meta_event_type === 'lifecycle') {
        // 处理生命周期事件
        const subType = metaEvent.sub_type || '未知';
        
        return `
          <div class="meta-event-content">
            <div class="meta-event-item">
              <span class="meta-event-label">子类型:</span>
              <span class="meta-event-value">${subType}</span>
            </div>
          </div>
        `;
      } else {
        // 对于未知类型的元事件，显示原始数据
        return `<pre class="raw-meta-data">${JSON.stringify(metaEvent, null, 2)}</pre>`;
      }
    } catch (e) {
      return `[元事件解析失败: ${(e as Error).message}]`;
    }
  }

  /**
   * 获取石头剪刀布的结果文本
   */
  private getRpsResult(result: any): string {
    const rpsMap: { [key: string]: string } = {
      '1': '石头',
      '2': '剪刀',
      '3': '布'
    };
    return rpsMap[result] || result;
  }

  /**
   * 渲染通知事件
   */
  private renderNoticeEvent(notice: any): string {
    if (!notice || !notice.notice_type) {
      return '[未知通知]';
    }

    const noticeType = notice.notice_type;
    const groupName = notice.group_name || (notice.group_id ? `群${notice.group_id}` : '');
    const userId = notice.user_id || '未知用户';
    
    switch (noticeType) {
      case 'group_increase':
        return `<span class="notice-group-increase">用户${userId}加入了${groupName || '群'}</span>`;
      case 'group_decrease':
        return `<span class="notice-group-decrease">用户${userId}离开了${groupName || '群'}</span>`;
      case 'group_admin':
        const setAdmin = notice.sub_type === 'set' ? '成为' : '不再是';
        return `<span class="notice-group-admin">用户${userId}${setAdmin}${groupName || '群'}的管理员</span>`;
      case 'group_upload':
        const fileName = notice.file ? (notice.file.name || notice.file.file || '未知文件') : '未知文件';
        return `<span class="notice-group-upload">用户${userId}在${groupName || '群'}上传了文件：${fileName}</span>`;
      case 'group_ban':
        const duration = notice.duration || 0;
        const action = duration > 0 ? `禁言${Math.floor(duration / 60)}分钟` : '解除禁言';
        return `<span class="notice-group-ban">用户${userId}在${groupName || '群'}被${action}</span>`;
      case 'friend_add':
        return `<span class="notice-friend-add">收到来自用户${userId}的好友请求</span>`;
      case 'group_recall':
        const msgId = notice.message_id || '未知';
        return `<span class="notice-group-recall">用户${userId}在${groupName || '群'}撤回了一条消息(ID: ${msgId})</span>`;
      case 'friend_recall':
        const fMsgId = notice.message_id || '未知';
        return `<span class="notice-friend-recall">用户${userId}撤回了一条消息(ID: ${fMsgId})</span>`;
      case 'notify':
        if (notice.sub_type === 'poke') {
          // 解析戳一戳事件
          const targetId = notice.target_id || '未知';
          // 构建戳一戳消息
          let pokeMessage = `用户${userId}戳了戳`;
          
          // 尝试从raw_info中提取更详细的信息
            if (notice.raw_info && Array.isArray(notice.raw_info)) {
              // 收集文本部分
              const textParts = notice.raw_info
                .filter((item: any) => item.type === 'nor' && item.txt)
                .map((item: any) => item.txt);
            
            if (textParts.length > 0) {
              // 如果有文本部分，使用它们构建更友好的消息
              pokeMessage = `${userId}${textParts.join('')}`;
            } else {
              // 否则使用默认格式
              pokeMessage += `用户${targetId}`;
            }
          } else {
            // 没有raw_info时使用简单格式
            pokeMessage += `用户${targetId}`;
          }
          
          if (groupName) {
            pokeMessage += ` (在${groupName})`;
          }
          
          return `<span class="notice-poke">${pokeMessage}</span>`;
        }
        // 其他通知子类型
        return `<span class="notice-notify">${notice.sub_type || '未知'}通知</span>`;
      default:
        // 显示通知类型和部分关键信息
        try {
          const keyInfo = [];
          if (userId) keyInfo.push(`用户: ${userId}`);
          if (groupName) keyInfo.push(`群: ${groupName}`);
          if (notice.sub_type) keyInfo.push(`子类型: ${notice.sub_type}`);
          const infoStr = keyInfo.length > 0 ? ` (${keyInfo.join(', ')})` : '';
          return `<span class="notice-unknown">${noticeType}通知${infoStr}</span>`;
        } catch (e) {
          return `[${noticeType}]`;
        }
    }
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
    if (this.totalMessagesCountElement) {
      this.totalMessagesCountElement.textContent = this.paginationInfo.total_logs.toString();
    }

    if (this.currentPageElement) {
      this.currentPageElement.textContent = this.currentPage.toString();
    }

    if (this.totalPagesElement) {
      this.totalPagesElement.textContent = this.paginationInfo.total_pages.toString();
    }
  }

  /**
   * 刷新消息数据
   */
  private refreshMessages(): void {
    this.fetchMessages();
  }

  /**
   * 跳转到上一页
   */
  private goToPrevPage(): void {
    if (this.currentPage > 1) {
      this.currentPage--;
      this.fetchMessages();
    }
  }

  /**
   * 跳转到下一页
   */
  private goToNextPage(): void {
    if (this.paginationInfo && this.currentPage < this.paginationInfo.total_pages) {
      this.currentPage++;
      this.fetchMessages();
    }
  }

  /**
   * 显示空状态
   */
  private showEmptyState(): void {
    if (!this.messagesContainer) return;

    // 检查是否已存在空状态元素
    let emptyElement = document.getElementById('empty-messages');
    if (!emptyElement) {
      emptyElement = document.createElement('div');
      emptyElement.id = 'empty-messages';
      emptyElement.className = 'empty-state';
      emptyElement.innerHTML = '<i class="fas fa-comment-slash"></i><p>暂无消息数据</p>';
      this.messagesContainer.appendChild(emptyElement);
    } else {
      emptyElement.style.display = 'block';
    }
  }

  /**
   * 隐藏空状态
   */
  private hideEmptyState(): void {
    const emptyElement = document.getElementById('empty-messages');
    if (emptyElement) {
      emptyElement.style.display = 'none';
    }
  }

  /**
   * 显示错误消息
   */
  private showErrorMessage(message: string): void {
    if (!this.messagesContainer) return;

    // 隐藏空状态
    this.hideEmptyState();

    // 创建错误消息元素
    let errorElement = document.createElement('div');
    errorElement.className = 'error-container';
    errorElement.innerHTML = `<p><i class="fas fa-exclamation-circle"></i> ${message}</p>`;

    // 清除之前的错误消息
    const oldErrorElement = this.messagesContainer.querySelector('.error-container');
    if (oldErrorElement) {
      this.messagesContainer.removeChild(oldErrorElement);
    }

    // 添加新的错误消息
    this.messagesContainer.appendChild(errorElement);

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
 * 立即初始化消息查看器
 * 不依赖DOMContentLoaded事件，确保在页面内容动态加载时也能正常初始化
 */
function initializeMessageViewer() {
  console.log("init");
  // 检查是否所有必要的DOM元素都已加载
  const requiredElements = [
    'messages-container',
    'refresh-messages-button',
    'toggle-auto-refresh-button',
    'page-info',
    'prev-page',
    'next-page'
  ];

  // 检查每个必要元素是否存在
  const allElementsExist = requiredElements.every(id => document.getElementById(id) !== null);

  // 如果所有元素都存在，初始化MessageViewer
  if (allElementsExist) {
    new MessageViewer();
  } else {
    // 如果有元素不存在，延迟一小段时间后重试
    setTimeout(initializeMessageViewer, 100);
  }
}

// 立即尝试初始化
initializeMessageViewer();
