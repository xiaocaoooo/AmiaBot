package main

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"regexp"
	"strconv"
	"strings"
	"sync"
	"time"

	hclog "github.com/hashicorp/go-hclog"
	"github.com/hashicorp/go-plugin"
	"github.com/xiaocaoooo/amiabot-plugin-sdk/onebot/ob11"
	papi "github.com/xiaocaoooo/amiabot-plugin-sdk/plugin"
	"github.com/xiaocaoooo/amiabot-plugin-sdk/plugin/transport"
	"github.com/xiaocaoooo/amiabot-plugin-sdk/util"
)

const (
	userCommandPattern  = `^(?:query|资料卡|用户资料)(?:\s+.*|\[CQ:at,[^\]]+\].*)?$`
	groupCommandPattern = `^(?:group|群资料卡|群信息)$`
	timeLayout          = "2006-01-02 15:04:05"
	textLimit           = 180
	signatureLimit      = 120
	noticeLimit         = 160
)

var rawAtRegex = regexp.MustCompile(`\[CQ:at,qq=(\d+)`)

type QueryPlugin struct {
	mu  sync.RWMutex
	cfg struct {
		AmiabotPages string `json:"amiabot_pages"`
	}
}

type eventContext struct {
	MsgType    string
	GroupID    any
	UserID     any
	RawMessage string
	Payload    map[string]any
}

type strangerInfo struct {
	UserID        int64  `json:"user_id"`
	UID           string `json:"uid"`
	Nickname      string `json:"nickname"`
	Nick          string `json:"nick"`
	Remark        string `json:"remark"`
	Sex           string `json:"sex"`
	Age           int    `json:"age"`
	QID           string `json:"qid"`
	QQLevel       int    `json:"qqLevel"`
	QQLevelAlt    int    `json:"qq_level"`
	Level         int    `json:"level"`
	LongNick      string `json:"long_nick"`
	LongNickAlt   string `json:"longNick"`
	RegTime       int64  `json:"reg_time"`
	RegTimeAlt    int64  `json:"regTime"`
	RegYear       int    `json:"reg_year"`
	IsVIP         *bool  `json:"is_vip"`
	IsYearsVIP    *bool  `json:"is_years_vip"`
	VIPLevel      int    `json:"vip_level"`
	Status        int    `json:"status"`
	LoginDays     int    `json:"login_days"`
	BirthdayYear  int    `json:"birthday_year"`
	BirthdayMonth int    `json:"birthday_month"`
	BirthdayDay   int    `json:"birthday_day"`
	PhoneNum      string `json:"phone_num"`
	Email         string `json:"email"`
	CategoryID    int    `json:"category_id"`
	CategoryIDAlt int    `json:"categoryId"`
	CategoryName  string `json:"categoryName"`
}

type groupMemberInfo struct {
	UserID          int64  `json:"user_id"`
	Nickname        string `json:"nickname"`
	Card            string `json:"card"`
	Sex             string `json:"sex"`
	Age             int    `json:"age"`
	Level           any    `json:"level"`
	JoinTime        int64  `json:"join_time"`
	LastSentTime    int64  `json:"last_sent_time"`
	QQLevel         int    `json:"qq_level"`
	Role            string `json:"role"`
	Title           string `json:"title"`
	Area            string `json:"area"`
	Unfriendly      *bool  `json:"unfriendly"`
	TitleExpireTime int64  `json:"title_expire_time"`
	CardChangeable  *bool  `json:"card_changeable"`
	ShutUpTimestamp int64  `json:"shut_up_timestamp"`
	IsRobot         *bool  `json:"is_robot"`
	QAge            any    `json:"qage"`
}

type groupMemberListItem struct {
	UserID          int64  `json:"user_id"`
	Nickname        string `json:"nickname"`
	Card            string `json:"card"`
	Sex             string `json:"sex"`
	LastSentTime    int64  `json:"last_sent_time"`
	Role            string `json:"role"`
	Title           string `json:"title"`
	Unfriendly      *bool  `json:"unfriendly"`
	ShutUpTimestamp int64  `json:"shut_up_timestamp"`
	IsRobot         *bool  `json:"is_robot"`
}

type groupDetailInfo struct {
	GroupID            int64  `json:"group_id"`
	GroupName          string `json:"group_name"`
	OwnerUin           int64  `json:"ownerUin"`
	OwnerID            int64  `json:"owner_id"`
	MemberNum          int    `json:"memberNum"`
	MemberCount        int    `json:"member_count"`
	MaxMemberNum       int    `json:"maxMemberNum"`
	MaxMemberCount     int    `json:"max_member_count"`
	GroupMemo          string `json:"groupMemo"`
	GroupCreateTime    int64  `json:"groupCreateTime"`
	GroupGrade         int    `json:"groupGrade"`
	ActiveMemberNum    int    `json:"activeMemberNum"`
	FingerMemo         string `json:"fingerMemo"`
	GroupQuestion      string `json:"groupQuestion"`
	ShutUpAllTimestamp int64  `json:"shutUpAllTimestamp"`
}

type groupInfo struct {
	GroupID        int64  `json:"group_id"`
	GroupName      string `json:"group_name"`
	MemberCount    int    `json:"member_count"`
	MaxMemberCount int    `json:"max_member_count"`
	GroupAllShut   int    `json:"group_all_shut"`
	GroupRemark    string `json:"group_remark"`
}

type profileLikeData struct {
	UID          string `json:"uid"`
	Time         string `json:"time"`
	FavoriteInfo struct {
		TotalCount int   `json:"total_count"`
		LastTime   int64 `json:"last_time"`
		TodayCount int   `json:"today_count"`
	} `json:"favoriteInfo"`
	VoteInfo struct {
		TotalCount    int   `json:"total_count"`
		NewCount      int   `json:"new_count"`
		LastVisitTime int64 `json:"last_visit_time"`
	} `json:"voteInfo"`
}

type userStatusData struct {
	Status    int `json:"status"`
	ExtStatus int `json:"ext_status"`
}

type groupHonorMember struct {
	UserID      int64  `json:"user_id"`
	Nickname    string `json:"nickname"`
	Description string `json:"description"`
}

type groupHonorData struct {
	GroupID          any                `json:"group_id"`
	CurrentTalkative groupHonorMember   `json:"current_talkative"`
	TalkativeList    []groupHonorMember `json:"talkative_list"`
	PerformerList    []groupHonorMember `json:"performer_list"`
	LegendList       []groupHonorMember `json:"legend_list"`
	EmotionList      []groupHonorMember `json:"emotion_list"`
	StrongNewbieList []groupHonorMember `json:"strong_newbie_list"`
}

type groupNoticeData struct {
	SenderID    int64  `json:"sender_id"`
	PublishTime int64  `json:"publish_time"`
	NoticeID    string `json:"notice_id"`
	Message     struct {
		Text   string   `json:"text"`
		Image  []string `json:"image"`
		Images []string `json:"images"`
	} `json:"message"`
	ReadNum int `json:"read_num"`
}

type groupAtAllRemainData struct {
	CanAtAll                 bool `json:"can_at_all"`
	RemainAtAllCountForGroup int  `json:"remain_at_all_count_for_group"`
	RemainAtAllCountForUin   int  `json:"remain_at_all_count_for_uin"`
}

type groupInfoExData struct {
	GroupCode  string `json:"groupCode"`
	ResultCode int    `json:"resultCode"`
	ExtInfo    struct {
		LuckyWord              string `json:"luckyWord"`
		HasGroupCustomPortrait int    `json:"hasGroupCustomPortrait"`
		BindGuildID            string `json:"bindGuildId"`
		GroupOwnerID           struct {
			MemberUin string `json:"memberUin"`
		} `json:"groupOwnerId"`
		EssentialMsgSwitch       int    `json:"essentialMsgSwitch"`
		InviteRobotSwitch        int    `json:"inviteRobotSwitch"`
		QQMusicMedalSwitch       int    `json:"qqMusicMedalSwitch"`
		ShowPlayTogetherSwitch   int    `json:"showPlayTogetherSwitch"`
		GroupBindGuildSwitch     int    `json:"groupBindGuildSwitch"`
		GroupAioBindGuildID      string `json:"groupAioBindGuildId"`
		FullGroupExpansionSwitch int    `json:"fullGroupExpansionSwitch"`
		InviteRobotMemberSwitch  int    `json:"inviteRobotMemberSwitch"`
		InviteRobotMemberExamine int    `json:"inviteRobotMemberExamine"`
		GroupSquareSwitch        int    `json:"groupSquareSwitch"`
		GroupExtFlameData        struct {
			SwitchState     int      `json:"switchState"`
			State           int      `json:"state"`
			DayNums         []string `json:"dayNums"`
			IsDisplayDayNum bool     `json:"isDisplayDayNum"`
		} `json:"groupExtFlameData"`
	} `json:"extInfo"`
}

type userPagePayload struct {
	ID               int64
	Nickname         string
	Remark           string
	UID              string
	QID              string
	LongNick         string
	Sex              string
	Age              int
	RegYear          int
	LoginDays        int
	QQLevel          int
	Role             string
	GroupLevel       string
	Title            string
	JoinTime         int64
	LastSentTime     int64
	Area             string
	QAge             string
	Birthday         string
	PhoneNum         string
	Email            string
	CategoryName     string
	CategoryID       string
	IsVIP            *bool
	IsYearsVIP       *bool
	VIPLevel         int
	OnlineStatus     int
	OnlineExtStatus  int
	MuteUntil        int64
	TitleExpireTime  int64
	Card             string
	CardChangeable   *bool
	Unfriendly       *bool
	IsRobot          *bool
	ProfileLikeTotal int
	ProfileLikeToday int
	ProfileLikeLast  int64
	VoteLikeTotal    int
	VoteLikeNew      int
	VoteVisitLast    int64
}

type groupPagePayload struct {
	ID                       int64
	Name                     string
	Remark                   string
	Level                    int
	CreateTime               int64
	MemberCount              int
	MaxMemberCount           int
	ActiveMembers            int
	DerivedActiveMembers     int
	OwnerID                  int64
	Rules                    string
	JoinQuestion             string
	IsMutedAll               bool
	Description              string
	AdminCount               int
	RobotCount               int
	MutedCount               int
	CardCount                int
	TitleCount               int
	UnfriendlyCount          int
	MaleCount                int
	FemaleCount              int
	UnknownSexCount          int
	CanAtAll                 *bool
	RemainAtAllCountForGroup int
	RemainAtAllCountForUin   int
	CurrentTalkative         string
	TalkativeTop             string
	PerformerTop             string
	LegendTop                string
	EmotionTop               string
	StrongNewbieTop          string
	LatestNoticeText         string
	LatestNoticeTime         int64
	LatestNoticeSenderID     int64
	LatestNoticeReadNum      int
	LatestNoticeImageCount   int
	LuckyWord                string
	HasGroupCustomPortrait   *bool
	BindGuildID              string
	GroupAioBindGuildID      string
	EssentialMsgSwitch       *bool
	InviteRobotSwitch        *bool
	QQMusicMedalSwitch       *bool
	ShowPlayTogetherSwitch   *bool
	GroupBindGuildSwitch     *bool
	FullGroupExpansionSwitch *bool
	InviteRobotMemberSwitch  *bool
	InviteRobotMemberExamine *bool
	GroupSquareSwitch        *bool
	FlameSwitchState         int
	FlameState               int
	FlameDayNums             string
	FlameDisplayDayNum       *bool
}

func (p *QueryPlugin) Descriptor(ctx context.Context) (papi.Descriptor, error) {
	_ = ctx
	schema := json.RawMessage(`{
		"type":"object",
		"properties":{
			"amiabot_pages":{"type":"string","description":"Amiabot Pages 域名/地址；为空则无法生成截图 URL"}
		},
		"additionalProperties":true
	}`)
	def := json.RawMessage(`{"amiabot_pages":""}`)

	return papi.Descriptor{
		Name:         "Amiabot Query",
		PluginID:     "external.amiabot-query",
		Version:      "0.2.0",
		Author:       "nyanyabot",
		Description:  "用户资料卡 / 群资料卡查询插件",
		Dependencies: []string{"external.screenshot", "external.blobserver"},
		Exports:      []papi.ExportSpec{},
		Config: &papi.ConfigSpec{
			Version:     "1",
			Description: "Amiabot Query plugin config",
			Schema:      schema,
			Default:     def,
		},
		Commands: []papi.CommandListener{
			{
				Name:        "query-user",
				ID:          "cmd.query-user",
				Description: "查询用户资料卡（如 query、query @某人、资料卡）",
				Pattern:     userCommandPattern,
				MatchRaw:    true,
				Handler:     "HandleQueryUser",
			},
			{
				Name:        "query-group",
				ID:          "cmd.query-group",
				Description: "查询当前群资料卡（如 group、群资料卡、群信息）",
				Pattern:     groupCommandPattern,
				MatchRaw:    true,
				Handler:     "HandleQueryGroup",
			},
		},
	}, nil
}

func (p *QueryPlugin) Configure(ctx context.Context, config json.RawMessage) error {
	_ = ctx
	cfg := struct {
		AmiabotPages string `json:"amiabot_pages"`
	}{}
	if len(config) > 0 {
		_ = json.Unmarshal(config, &cfg)
	}
	p.mu.Lock()
	p.cfg.AmiabotPages = strings.TrimSpace(cfg.AmiabotPages)
	p.mu.Unlock()
	return nil
}

func (p *QueryPlugin) Invoke(ctx context.Context, method string, paramsJSON json.RawMessage, callerPluginID string) (json.RawMessage, error) {
	_ = ctx
	_ = method
	_ = paramsJSON
	_ = callerPluginID
	return nil, papi.NewStructuredError(papi.ErrorCodeNotFound, "method is not exported")
}

func (p *QueryPlugin) Handle(ctx context.Context, listenerID string, eventRaw ob11.Event, match *papi.CommandMatch) (papi.HandleResult, error) {
	switch listenerID {
	case "cmd.query-user":
		return p.handleQueryUser(ctx, eventRaw, match)
	case "cmd.query-group":
		return p.handleQueryGroup(ctx, eventRaw)
	default:
		return papi.HandleResult{}, nil
	}
}

func (p *QueryPlugin) Shutdown(ctx context.Context) error {
	_ = ctx
	return nil
}

func (p *QueryPlugin) handleQueryUser(ctx context.Context, eventRaw ob11.Event, match *papi.CommandMatch) (papi.HandleResult, error) {
	_ = match
	log := hclog.L()
	evt, err := parseEventContext(eventRaw)
	if err != nil {
		log.Error("[Query] 解析用户查询事件失败", "error", err)
		return papi.HandleResult{}, nil
	}

	host := transport.Host()
	if host == nil {
		log.Warn("[Query] host 为 nil，终止用户查询")
		return papi.HandleResult{}, nil
	}

	targetUserID := extractTargetUserID(evt.Payload, evt.RawMessage)
	if targetUserID <= 0 {
		targetUserID = anyToInt64(evt.Payload["user_id"])
	}
	if targetUserID <= 0 {
		util.SendText(host, evt.MsgType, evt.GroupID, evt.UserID, "❌ 无法识别要查询的用户")
		return papi.HandleResult{}, nil
	}

	stranger, err := callOneBotJSON[strangerInfo](ctx, host, "get_stranger_info", map[string]any{
		"user_id":  targetUserID,
		"no_cache": false,
	})
	if err != nil {
		log.Error("[Query] 获取用户资料失败", "target_user_id", targetUserID, "error", err)
		util.SendError(host, evt.MsgType, evt.GroupID, evt.UserID, "❌ 获取用户资料失败", err)
		return papi.HandleResult{}, nil
	}

	payload := userPagePayload{
		ID:           targetUserID,
		Nickname:     firstNonEmpty(stranger.Nickname, stranger.Nick, strconv.FormatInt(targetUserID, 10)),
		Remark:       strings.TrimSpace(stranger.Remark),
		UID:          strings.TrimSpace(stranger.UID),
		QID:          strings.TrimSpace(stranger.QID),
		LongNick:     truncateText(firstNonEmpty(stranger.LongNick, stranger.LongNickAlt), signatureLimit),
		Sex:          normalizeEnum(stranger.Sex),
		Age:          stranger.Age,
		RegYear:      normalizeRegYear(stranger.RegYear, firstPositiveInt64(stranger.RegTime, stranger.RegTimeAlt)),
		LoginDays:    stranger.LoginDays,
		QQLevel:      firstPositive(stranger.QQLevel, stranger.QQLevelAlt, stranger.Level),
		Birthday:     formatBirthday(stranger.BirthdayYear, stranger.BirthdayMonth, stranger.BirthdayDay),
		PhoneNum:     strings.TrimSpace(stranger.PhoneNum),
		Email:        strings.TrimSpace(stranger.Email),
		CategoryName: strings.TrimSpace(stranger.CategoryName),
		CategoryID:   intToString(firstPositive(stranger.CategoryID, stranger.CategoryIDAlt)),
		IsVIP:        stranger.IsVIP,
		IsYearsVIP:   stranger.IsYearsVIP,
		VIPLevel:     stranger.VIPLevel,
		OnlineStatus: stranger.Status,
	}

	if evt.MsgType == "group" {
		groupID := anyToInt64(evt.Payload["group_id"])
		if groupID > 0 {
			member, memberErr := callOneBotJSON[groupMemberInfo](ctx, host, "get_group_member_info", map[string]any{
				"group_id": groupID,
				"user_id":  targetUserID,
				"no_cache": false,
			})
			if memberErr != nil {
				log.Warn("[Query] 获取群成员资料失败，继续返回基础资料", "group_id", groupID, "target_user_id", targetUserID, "error", memberErr)
			} else {
				payload.Card = strings.TrimSpace(member.Card)
				payload.Role = normalizeEnum(member.Role)
				payload.GroupLevel = anyToString(member.Level)
				payload.Title = strings.TrimSpace(member.Title)
				payload.JoinTime = member.JoinTime
				payload.LastSentTime = member.LastSentTime
				payload.Area = strings.TrimSpace(member.Area)
				payload.QAge = normalizeQAge(member.QAge)
				payload.MuteUntil = member.ShutUpTimestamp
				payload.TitleExpireTime = member.TitleExpireTime
				payload.CardChangeable = member.CardChangeable
				payload.Unfriendly = member.Unfriendly
				payload.IsRobot = member.IsRobot
				payload.QQLevel = firstPositive(payload.QQLevel, member.QQLevel)
				if payload.Nickname == "" {
					payload.Nickname = firstNonEmpty(member.Card, member.Nickname, strconv.FormatInt(targetUserID, 10))
				}
				if payload.Sex == "" {
					payload.Sex = normalizeEnum(member.Sex)
				}
				if payload.Age <= 0 {
					payload.Age = member.Age
				}
			}
		}
	}

	if statusInfo, statusErr := callOneBotJSON[userStatusData](ctx, host, "nc_get_user_status", map[string]any{"user_id": targetUserID}); statusErr == nil {
		payload.OnlineStatus = statusInfo.Status
		payload.OnlineExtStatus = statusInfo.ExtStatus
	} else {
		log.Warn("[Query] 获取用户在线状态失败，忽略", "target_user_id", targetUserID, "error", statusErr)
	}

	pageURL, err := p.buildUserPageURL(payload)
	if err != nil {
		util.SendError(host, evt.MsgType, evt.GroupID, evt.UserID, "❌ 服务未配置", err)
		return papi.HandleResult{}, nil
	}
	if sendErr := sendScreenshotPage(ctx, host, evt, pageURL, fmt.Sprintf("query-user-%d-%d", targetUserID, time.Now().Unix())); sendErr != nil {
		util.SendError(host, evt.MsgType, evt.GroupID, evt.UserID, "❌ 资料卡生成失败", sendErr)
	}
	return papi.HandleResult{}, nil
}

func (p *QueryPlugin) handleQueryGroup(ctx context.Context, eventRaw ob11.Event) (papi.HandleResult, error) {
	log := hclog.L()
	evt, err := parseEventContext(eventRaw)
	if err != nil {
		log.Error("[Query] 解析群查询事件失败", "error", err)
		return papi.HandleResult{}, nil
	}

	host := transport.Host()
	if host == nil {
		log.Warn("[Query] host 为 nil，终止群查询")
		return papi.HandleResult{}, nil
	}
	if evt.MsgType != "group" {
		util.SendText(host, evt.MsgType, evt.GroupID, evt.UserID, "该命令只能在群聊中使用")
		return papi.HandleResult{}, nil
	}

	groupID := anyToInt64(evt.Payload["group_id"])
	if groupID <= 0 {
		util.SendText(host, evt.MsgType, evt.GroupID, evt.UserID, "❌ 无法识别当前群聊")
		return papi.HandleResult{}, nil
	}

	payload, err := fetchGroupPayload(ctx, host, groupID)
	if err != nil {
		log.Error("[Query] 获取群资料失败", "group_id", groupID, "error", err)
		util.SendError(host, evt.MsgType, evt.GroupID, evt.UserID, "❌ 获取群资料失败", err)
		return papi.HandleResult{}, nil
	}

	pageURL, err := p.buildGroupPageURL(payload)
	if err != nil {
		util.SendError(host, evt.MsgType, evt.GroupID, evt.UserID, "❌ 服务未配置", err)
		return papi.HandleResult{}, nil
	}
	if sendErr := sendScreenshotPage(ctx, host, evt, pageURL, fmt.Sprintf("query-group-%d-%d", groupID, time.Now().Unix())); sendErr != nil {
		util.SendError(host, evt.MsgType, evt.GroupID, evt.UserID, "❌ 群资料卡生成失败", sendErr)
	}
	return papi.HandleResult{}, nil
}

func (p *QueryPlugin) buildUserPageURL(payload userPagePayload) (string, error) {
	p.mu.RLock()
	pagesHost := p.cfg.AmiabotPages
	p.mu.RUnlock()
	if pagesHost == "" {
		return "", fmt.Errorf("amiabot_pages 未配置")
	}
	params := buildUserPageParams(payload)
	return util.BuildPagesURL(pagesHost, "/query/user", params), nil
}

func (p *QueryPlugin) buildGroupPageURL(payload groupPagePayload) (string, error) {
	p.mu.RLock()
	pagesHost := p.cfg.AmiabotPages
	p.mu.RUnlock()
	if pagesHost == "" {
		return "", fmt.Errorf("amiabot_pages 未配置")
	}
	params := buildGroupPageParams(payload)
	return util.BuildPagesURL(pagesHost, "/query/group", params), nil
}

func parseEventContext(eventRaw ob11.Event) (eventContext, error) {
	var evt map[string]any
	dec := json.NewDecoder(bytes.NewReader(eventRaw))
	dec.UseNumber()
	if err := dec.Decode(&evt); err != nil {
		return eventContext{}, err
	}
	return eventContext{
		MsgType:    strings.TrimSpace(anyToString(evt["message_type"])),
		GroupID:    evt["group_id"],
		UserID:     evt["user_id"],
		RawMessage: strings.TrimSpace(anyToString(evt["raw_message"])),
		Payload:    evt,
	}, nil
}

func extractTargetUserID(evt map[string]any, rawMessage string) int64 {
	if evt == nil {
		return extractAtFromRaw(rawMessage)
	}
	if message, ok := evt["message"]; ok {
		if qq := extractAtFromSegments(message); qq > 0 {
			return qq
		}
	}
	return extractAtFromRaw(rawMessage)
}

func extractAtFromSegments(message any) int64 {
	switch items := message.(type) {
	case []any:
		for _, item := range items {
			if qq := extractAtFromSegment(item); qq > 0 {
				return qq
			}
		}
	case []map[string]any:
		for _, item := range items {
			if qq := extractAtFromSegment(item); qq > 0 {
				return qq
			}
		}
	}
	return 0
}

func extractAtFromSegment(segment any) int64 {
	segMap, ok := segment.(map[string]any)
	if !ok {
		return 0
	}
	if !strings.EqualFold(anyToString(segMap["type"]), "at") {
		return 0
	}
	data, ok := segMap["data"].(map[string]any)
	if !ok {
		return 0
	}
	qq := strings.TrimSpace(anyToString(data["qq"]))
	if qq == "" || qq == "all" {
		return 0
	}
	id, _ := strconv.ParseInt(qq, 10, 64)
	return id
}

func extractAtFromRaw(rawMessage string) int64 {
	m := rawAtRegex.FindStringSubmatch(rawMessage)
	if len(m) != 2 {
		return 0
	}
	id, _ := strconv.ParseInt(m[1], 10, 64)
	return id
}

func fetchGroupPayload(ctx context.Context, host util.HostCaller, groupID int64) (groupPagePayload, error) {
	log := hclog.L()
	payload := groupPagePayload{ID: groupID}

	detail, detailErr := callOneBotJSON[groupDetailInfo](ctx, host, "get_group_detail_info", map[string]any{"group_id": groupID})
	if detailErr == nil {
		payload.Name = firstNonEmpty(detail.GroupName, payload.Name)
		payload.Level = detail.GroupGrade
		payload.CreateTime = detail.GroupCreateTime
		payload.MemberCount = firstPositive(detail.MemberNum, detail.MemberCount)
		payload.MaxMemberCount = firstPositive(detail.MaxMemberNum, detail.MaxMemberCount)
		payload.ActiveMembers = detail.ActiveMemberNum
		payload.OwnerID = firstPositiveInt64(detail.OwnerUin, detail.OwnerID)
		payload.Rules = truncateText(detail.FingerMemo, textLimit)
		payload.JoinQuestion = truncateText(detail.GroupQuestion, textLimit)
		payload.IsMutedAll = detail.ShutUpAllTimestamp > 0
		payload.Description = truncateText(detail.GroupMemo, textLimit)
	}

	baseInfo, baseErr := callOneBotJSON[groupInfo](ctx, host, "get_group_info", map[string]any{"group_id": groupID, "no_cache": false})
	if baseErr == nil {
		payload.Name = firstNonEmpty(payload.Name, baseInfo.GroupName, fmt.Sprintf("群聊 %d", groupID))
		payload.MemberCount = firstPositive(payload.MemberCount, baseInfo.MemberCount)
		payload.MaxMemberCount = firstPositive(payload.MaxMemberCount, baseInfo.MaxMemberCount)
		payload.Remark = truncateText(baseInfo.GroupRemark, 64)
		if detailErr != nil {
			payload.IsMutedAll = baseInfo.GroupAllShut > 0
		}
	}

	if payload.Name == "" {
		payload.Name = fmt.Sprintf("群聊 %d", groupID)
	}
	if detailErr != nil && baseErr != nil {
		return groupPagePayload{}, fmt.Errorf("get_group_detail_info 失败: %w；get_group_info 也失败: %w", detailErr, baseErr)
	}

	if members, memberErr := callOneBotJSON[[]groupMemberListItem](ctx, host, "get_group_member_list", map[string]any{"group_id": groupID, "no_cache": false}); memberErr == nil {
		now := time.Now().Unix()
		for _, member := range members {
			switch normalizeEnum(member.Role) {
			case "owner":
				payload.OwnerID = firstPositiveInt64(payload.OwnerID, member.UserID)
			case "admin":
				payload.AdminCount++
			}
			if member.IsRobot != nil && *member.IsRobot {
				payload.RobotCount++
			}
			if member.Unfriendly != nil && *member.Unfriendly {
				payload.UnfriendlyCount++
			}
			if member.ShutUpTimestamp > now {
				payload.MutedCount++
			}
			if strings.TrimSpace(member.Card) != "" {
				payload.CardCount++
			}
			if strings.TrimSpace(member.Title) != "" {
				payload.TitleCount++
			}
			switch normalizeEnum(member.Sex) {
			case "male":
				payload.MaleCount++
			case "female":
				payload.FemaleCount++
			default:
				payload.UnknownSexCount++
			}
			if member.LastSentTime > now-7*24*3600 {
				payload.DerivedActiveMembers++
			}
		}
	} else {
		log.Warn("[Query] 获取群成员列表失败，部分统计将缺失", "group_id", groupID, "error", memberErr)
	}

	if honors, honorErr := callOneBotJSON[groupHonorData](ctx, host, "get_group_honor_info", map[string]any{"group_id": groupID, "type": "all"}); honorErr == nil {
		payload.CurrentTalkative = renderHonorMember(honors.CurrentTalkative)
		payload.TalkativeTop = renderHonorMember(firstHonorMember(honors.TalkativeList))
		payload.PerformerTop = renderHonorMember(firstHonorMember(honors.PerformerList))
		payload.LegendTop = renderHonorMember(firstHonorMember(honors.LegendList))
		payload.EmotionTop = renderHonorMember(firstHonorMember(honors.EmotionList))
		payload.StrongNewbieTop = renderHonorMember(firstHonorMember(honors.StrongNewbieList))
	} else {
		log.Warn("[Query] 获取群荣誉失败，忽略", "group_id", groupID, "error", honorErr)
	}

	if notices, noticeErr := callOneBotJSON[[]groupNoticeData](ctx, host, "_get_group_notice", map[string]any{"group_id": groupID}); noticeErr == nil && len(notices) > 0 {
		latest := notices[0]
		payload.LatestNoticeText = truncateText(strings.TrimSpace(latest.Message.Text), noticeLimit)
		payload.LatestNoticeTime = latest.PublishTime
		payload.LatestNoticeSenderID = latest.SenderID
		payload.LatestNoticeReadNum = latest.ReadNum
		payload.LatestNoticeImageCount = max(len(latest.Message.Image), len(latest.Message.Images))
	} else if noticeErr != nil {
		log.Warn("[Query] 获取群公告失败，忽略", "group_id", groupID, "error", noticeErr)
	}

	if infoEx, exErr := callOneBotJSON[groupInfoExData](ctx, host, "get_group_info_ex", map[string]any{"group_id": groupID}); exErr == nil {
		payload.LuckyWord = truncateText(infoEx.ExtInfo.LuckyWord, 32)
		payload.OwnerID = firstPositiveInt64(payload.OwnerID, stringToInt64(infoEx.ExtInfo.GroupOwnerID.MemberUin))
	} else {
		log.Warn("[Query] 获取群扩展信息失败，忽略", "group_id", groupID, "error", exErr)
	}

	return payload, nil
}

func buildUserPageParams(payload userPagePayload) map[string]string {
	params := map[string]string{}
	setInt64Param(params, "id", payload.ID)
	setStringParam(params, "nickname", payload.Nickname)
	setStringParam(params, "remark", payload.Remark)
	setStringParam(params, "qid", payload.QID)
	setStringParam(params, "long_nick", payload.LongNick)
	setStringParam(params, "sex", payload.Sex)
	setIntParam(params, "age", payload.Age)
	setIntParam(params, "reg_year", payload.RegYear)
	setIntParam(params, "qq_level", payload.QQLevel)
	setStringParam(params, "role", payload.Role)
	setStringParam(params, "group_level", payload.GroupLevel)
	setStringParam(params, "title", truncateText(payload.Title, 48))
	setStringParam(params, "join_time", formatTimestamp(payload.JoinTime))
	setStringParam(params, "last_sent_time", formatTimestamp(payload.LastSentTime))
	setStringParam(params, "area", payload.Area)
	setStringParam(params, "qage", payload.QAge)
	setStringParam(params, "birthday", payload.Birthday)
	setStringParam(params, "phone_num", payload.PhoneNum)
	setStringParam(params, "email", payload.Email)
	setStringParam(params, "category_name", payload.CategoryName)
	setStringParam(params, "category_id", payload.CategoryID)
	if payload.IsVIP != nil {
		setStringParam(params, "is_vip", strconv.FormatBool(*payload.IsVIP))
	}
	if payload.IsYearsVIP != nil {
		setStringParam(params, "is_years_vip", strconv.FormatBool(*payload.IsYearsVIP))
	}
	setIntParam(params, "vip_level", payload.VIPLevel)
	setIntParam(params, "online_status", payload.OnlineStatus)
	setIntParam(params, "online_ext_status", payload.OnlineExtStatus)
	setStringParam(params, "mute_until", formatTimestamp(payload.MuteUntil))
	setStringParam(params, "title_expire_time", formatTimestamp(payload.TitleExpireTime))
	setStringParam(params, "card", payload.Card)
	if payload.Unfriendly != nil {
		setStringParam(params, "unfriendly", strconv.FormatBool(*payload.Unfriendly))
	}
	if payload.IsRobot != nil {
		setStringParam(params, "is_robot", strconv.FormatBool(*payload.IsRobot))
	}
	return params
}

func buildGroupPageParams(payload groupPagePayload) map[string]string {
	params := map[string]string{}
	setInt64Param(params, "id", payload.ID)
	setStringParam(params, "name", payload.Name)
	setStringParam(params, "remark", payload.Remark)
	setIntParam(params, "level", payload.Level)
	setStringParam(params, "create_time", formatTimestamp(payload.CreateTime))
	setIntParam(params, "member_count", payload.MemberCount)
	setIntParam(params, "max_member_count", payload.MaxMemberCount)
	setIntParam(params, "active_member_count", payload.ActiveMembers)
	setIntParam(params, "derived_active_member_count", payload.DerivedActiveMembers)
	setInt64Param(params, "owner_id", payload.OwnerID)
	setStringParam(params, "rules", truncateText(payload.Rules, textLimit))
	setStringParam(params, "join_question", truncateText(payload.JoinQuestion, textLimit))
	setStringParam(params, "description", truncateText(payload.Description, textLimit))
	setStringParam(params, "is_muted_all", strconv.FormatBool(payload.IsMutedAll))
	setIntParam(params, "admin_count", payload.AdminCount)
	setIntParam(params, "robot_count", payload.RobotCount)
	setIntParam(params, "muted_count", payload.MutedCount)
	setIntParam(params, "card_count", payload.CardCount)
	setIntParam(params, "title_count", payload.TitleCount)
	setIntParam(params, "unfriendly_count", payload.UnfriendlyCount)
	setIntParam(params, "male_count", payload.MaleCount)
	setIntParam(params, "female_count", payload.FemaleCount)
	setIntParam(params, "unknown_sex_count", payload.UnknownSexCount)
	setStringParam(params, "current_talkative", payload.CurrentTalkative)
	setStringParam(params, "talkative_top", payload.TalkativeTop)
	setStringParam(params, "performer_top", payload.PerformerTop)
	setStringParam(params, "legend_top", payload.LegendTop)
	setStringParam(params, "emotion_top", payload.EmotionTop)
	setStringParam(params, "strong_newbie_top", payload.StrongNewbieTop)
	setStringParam(params, "latest_notice_text", truncateText(payload.LatestNoticeText, noticeLimit))
	setStringParam(params, "latest_notice_time", formatTimestamp(payload.LatestNoticeTime))
	setInt64Param(params, "latest_notice_sender_id", payload.LatestNoticeSenderID)
	setIntParam(params, "latest_notice_read_num", payload.LatestNoticeReadNum)
	setStringParam(params, "lucky_word", payload.LuckyWord)
	return params
}

func sendScreenshotPage(ctx context.Context, host util.HostCaller, evt eventContext, pageURL string, blobID string) error {
	screenshotURL, err := util.BuildScreenshotViaPlugin(host, pageURL)
	if err != nil {
		return err
	}
	if uploaded := util.UploadViaBlobPlugin(ctx, host, screenshotURL, blobID, "image"); uploaded != "" {
		screenshotURL = uploaded
	}
	return util.SendImage(host, evt.MsgType, evt.GroupID, evt.UserID, screenshotURL)
}

func callOneBotJSON[T any](ctx context.Context, host util.HostCaller, action string, params any) (T, error) {
	var zero T
	resp, err := host.CallOneBot(ctx, action, params)
	if err != nil {
		return zero, err
	}
	if resp.RetCode != 0 || (resp.Status != "" && !strings.EqualFold(resp.Status, "ok")) {
		return zero, fmt.Errorf("%s 返回失败: %s", action, firstNonEmpty(resp.Wording, resp.Msg, resp.Status, strconv.Itoa(resp.RetCode)))
	}
	if len(resp.Data) == 0 {
		return zero, fmt.Errorf("%s 返回空数据", action)
	}
	var out T
	dec := json.NewDecoder(bytes.NewReader(resp.Data))
	dec.UseNumber()
	if err := dec.Decode(&out); err != nil {
		return zero, fmt.Errorf("解析 %s 返回失败: %w", action, err)
	}
	return out, nil
}

func renderHonorMember(member groupHonorMember) string {
	if member.UserID <= 0 && strings.TrimSpace(member.Nickname) == "" {
		return ""
	}
	parts := make([]string, 0, 2)
	if strings.TrimSpace(member.Nickname) != "" {
		parts = append(parts, strings.TrimSpace(member.Nickname))
	}
	if member.UserID > 0 {
		parts = append(parts, strconv.FormatInt(member.UserID, 10))
	}
	result := strings.Join(parts, " · ")
	if desc := strings.TrimSpace(member.Description); desc != "" {
		result += "（" + desc + "）"
	}
	return result
}

func firstHonorMember(items []groupHonorMember) groupHonorMember {
	if len(items) == 0 {
		return groupHonorMember{}
	}
	return items[0]
}

func formatBirthday(year, month, day int) string {
	if year <= 0 || month <= 0 || day <= 0 {
		return ""
	}
	return fmt.Sprintf("%04d-%02d-%02d", year, month, day)
}

func normalizeRegYear(regYear int, regTime int64) int {
	if regYear > 0 {
		return regYear
	}
	if regTime > 0 {
		return time.Unix(regTime, 0).Local().Year()
	}
	return 0
}

func formatTimestamp(ts int64) string {
	if ts <= 0 {
		return ""
	}
	if ts > 1_000_000_000_000 {
		return time.UnixMilli(ts).Local().Format(timeLayout)
	}
	return time.Unix(ts, 0).Local().Format(timeLayout)
}

func normalizeQAge(v any) string {
	text := strings.TrimSpace(anyToString(v))
	if text == "" || text == "0" {
		return ""
	}
	if !strings.Contains(text, "年") {
		return text + " 年"
	}
	return text
}

func truncateText(s string, limit int) string {
	s = strings.TrimSpace(s)
	if s == "" || limit <= 0 {
		return s
	}
	runes := []rune(s)
	if len(runes) <= limit {
		return s
	}
	return strings.TrimSpace(string(runes[:limit])) + "…"
}

func normalizeEnum(s string) string {
	return strings.ToLower(strings.TrimSpace(s))
}

func setStringParam(params map[string]string, key, value string) {
	value = strings.TrimSpace(value)
	if value == "" {
		return
	}
	params[key] = value
}

func setIntParam(params map[string]string, key string, value int) {
	if value <= 0 {
		return
	}
	params[key] = strconv.Itoa(value)
}

func setInt64Param(params map[string]string, key string, value int64) {
	if value <= 0 {
		return
	}
	params[key] = strconv.FormatInt(value, 10)
}

func anyToString(v any) string {
	switch x := v.(type) {
	case string:
		return x
	case json.Number:
		return x.String()
	case float64:
		return strconv.FormatFloat(x, 'f', -1, 64)
	case float32:
		return strconv.FormatFloat(float64(x), 'f', -1, 32)
	case int:
		return strconv.Itoa(x)
	case int64:
		return strconv.FormatInt(x, 10)
	case int32:
		return strconv.FormatInt(int64(x), 10)
	case uint64:
		return strconv.FormatUint(x, 10)
	case uint32:
		return strconv.FormatUint(uint64(x), 10)
	case bool:
		return strconv.FormatBool(x)
	default:
		return ""
	}
}

func anyToInt64(v any) int64 {
	switch x := v.(type) {
	case int64:
		return x
	case int:
		return int64(x)
	case int32:
		return int64(x)
	case float64:
		return int64(x)
	case json.Number:
		n, _ := x.Int64()
		return n
	case string:
		n, _ := strconv.ParseInt(strings.TrimSpace(x), 10, 64)
		return n
	default:
		return 0
	}
}

func intToString(v int) string {
	if v <= 0 {
		return ""
	}
	return strconv.Itoa(v)
}

func stringToInt64(v string) int64 {
	n, _ := strconv.ParseInt(strings.TrimSpace(v), 10, 64)
	return n
}

func firstNonEmpty(values ...string) string {
	for _, value := range values {
		value = strings.TrimSpace(value)
		if value != "" {
			return value
		}
	}
	return ""
}

func firstPositive(values ...int) int {
	for _, value := range values {
		if value > 0 {
			return value
		}
	}
	return 0
}

func firstPositiveInt64(values ...int64) int64 {
	for _, value := range values {
		if value > 0 {
			return value
		}
	}
	return 0
}

func boolPtr(v bool) *bool {
	b := v
	return &b
}

func boolPtrFromInt(v int) *bool {
	if v == 0 {
		return boolPtr(false)
	}
	return boolPtr(true)
}

func max(a, b int) int {
	if a > b {
		return a
	}
	return b
}

func main() {
	plugin.Serve(&plugin.ServeConfig{
		HandshakeConfig: transport.Handshake(),
		Plugins: plugin.PluginSet{
			transport.PluginName: &transport.Map{PluginImpl: &QueryPlugin{}},
		},
	})
}
