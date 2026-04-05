package main

import (
	"context"
	"encoding/json"
	"strings"
	"testing"

	"github.com/xiaocaoooo/amiabot-plugin-sdk/onebot/ob11"
)

type stubHost struct {
	responses map[string]ob11.APIResponse
}

func (s *stubHost) CallOneBot(ctx context.Context, action string, params any) (ob11.APIResponse, error) {
	_ = ctx
	_ = params
	if resp, ok := s.responses[action]; ok {
		return resp, nil
	}
	return ob11.APIResponse{}, nil
}

func (s *stubHost) CallDependency(ctx context.Context, targetPluginID string, method string, params any) (json.RawMessage, error) {
	_ = ctx
	_ = targetPluginID
	_ = method
	_ = params
	return nil, nil
}

func TestExtractTargetUserIDFromSegments(t *testing.T) {
	evt := map[string]any{
		"message": []any{
			map[string]any{"type": "text", "data": map[string]any{"text": "query "}},
			map[string]any{"type": "at", "data": map[string]any{"qq": "123456"}},
		},
	}

	if got := extractTargetUserID(evt, "query [CQ:at,qq=654321]"); got != 123456 {
		t.Fatalf("extractTargetUserID() = %d, want %d", got, 123456)
	}
}

func TestExtractTargetUserIDFallsBackToRawMessage(t *testing.T) {
	if got := extractTargetUserID(nil, "query[CQ:at,qq=654321]"); got != 654321 {
		t.Fatalf("extractTargetUserID() = %d, want %d", got, 654321)
	}
}

func TestBuildUserPageParamsIncludesExtendedFields(t *testing.T) {
	isVIP := true
	isYearsVIP := false
	cardChangeable := true
	unfriendly := false
	isRobot := true
	params := buildUserPageParams(userPagePayload{
		ID:               12345,
		Nickname:         "测试用户",
		Remark:           "好友备注",
		UID:              "u_123",
		QID:              "qid-001",
		LongNick:         "个性签名",
		Sex:              "female",
		Age:              18,
		RegYear:          2020,
		LoginDays:        365,
		QQLevel:          64,
		Role:             "admin",
		GroupLevel:       "火花",
		Title:            "测试头衔",
		JoinTime:         1700000000,
		LastSentTime:     1700000100,
		Area:             "上海",
		QAge:             "8 年",
		Birthday:         "2000-01-01",
		PhoneNum:         "1234567890",
		Email:            "test@example.com",
		CategoryName:     "特别关注",
		CategoryID:       "12",
		IsVIP:            &isVIP,
		IsYearsVIP:       &isYearsVIP,
		VIPLevel:         7,
		OnlineStatus:     10,
		OnlineExtStatus:  1028,
		MuteUntil:        1700000200,
		TitleExpireTime:  1700000300,
		Card:             "群名片",
		CardChangeable:   &cardChangeable,
		Unfriendly:       &unfriendly,
		IsRobot:          &isRobot,
		ProfileLikeTotal: 99,
		ProfileLikeToday: 3,
		ProfileLikeLast:  1700000400,
		VoteLikeTotal:    120,
		VoteLikeNew:      5,
		VoteVisitLast:    1700000500,
	})

	for key, want := range map[string]string{
		"id":                "12345",
		"remark":            "好友备注",
		"qid":               "qid-001",
		"long_nick":         "个性签名",
		"area":              "上海",
		"qage":              "8 年",
		"birthday":          "2000-01-01",
		"phone_num":         "1234567890",
		"email":             "test@example.com",
		"category_name":     "特别关注",
		"online_status":     "10",
		"online_ext_status": "1028",
		"unfriendly":        "false",
		"is_robot":          "true",
	} {

		if got := params[key]; got != want {
			t.Fatalf("params[%q] = %q, want %q", key, got, want)
		}
	}
	if got := params["mute_until"]; !strings.Contains(got, "-") {
		t.Fatalf("mute_until not formatted: %q", got)
	}
	for _, removedKey := range []string{"uid", "login_days", "card_changeable", "profile_like_total", "profile_like_today", "profile_like_last", "vote_like_total", "vote_like_new", "vote_visit_last"} {
		if _, exists := params[removedKey]; exists {
			t.Fatalf("params should not contain %q", removedKey)
		}
	}
}

func TestBuildGroupPageParamsIncludesExtendedFields(t *testing.T) {
	canAtAll := true
	hasPortrait := true
	flagFalse := false
	params := buildGroupPageParams(groupPagePayload{
		ID:                       123,
		Name:                     "测试群",
		Remark:                   "群备注",
		Level:                    5,
		CreateTime:               1700000000,
		MemberCount:              100,
		MaxMemberCount:           200,
		ActiveMembers:            66,
		DerivedActiveMembers:     51,
		OwnerID:                  9988,
		Rules:                    "群规",
		JoinQuestion:             "问题",
		IsMutedAll:               true,
		Description:              "简介",
		AdminCount:               3,
		RobotCount:               2,
		MutedCount:               4,
		CardCount:                40,
		TitleCount:               8,
		UnfriendlyCount:          1,
		MaleCount:                30,
		FemaleCount:              20,
		UnknownSexCount:          10,
		CanAtAll:                 &canAtAll,
		RemainAtAllCountForGroup: 5,
		RemainAtAllCountForUin:   2,
		CurrentTalkative:         "龙王A",
		TalkativeTop:             "群聊之火A",
		PerformerTop:             "群聊炽焰A",
		LegendTop:                "传奇A",
		EmotionTop:               "快乐源泉A",
		StrongNewbieTop:          "新人A",
		LatestNoticeText:         strings.Repeat("公告", 100),
		LatestNoticeTime:         1700000100,
		LatestNoticeSenderID:     12345,
		LatestNoticeReadNum:      80,
		LatestNoticeImageCount:   2,
		LuckyWord:                "幸运字符",
		HasGroupCustomPortrait:   &hasPortrait,
		BindGuildID:              "guild-1",
		GroupAioBindGuildID:      "guild-aio-1",
		EssentialMsgSwitch:       &flagFalse,
		InviteRobotSwitch:        &hasPortrait,
		QQMusicMedalSwitch:       &flagFalse,
		ShowPlayTogetherSwitch:   &hasPortrait,
		GroupBindGuildSwitch:     &hasPortrait,
		FullGroupExpansionSwitch: &flagFalse,
		InviteRobotMemberSwitch:  &hasPortrait,
		InviteRobotMemberExamine: &flagFalse,
		GroupSquareSwitch:        &hasPortrait,
		FlameSwitchState:         1,
		FlameState:               2,
		FlameDayNums:             "1,2,3",
		FlameDisplayDayNum:       &hasPortrait,
	})

	for key, want := range map[string]string{
		"remark":                  "群备注",
		"admin_count":             "3",
		"robot_count":             "2",
		"current_talkative":       "龙王A",
		"latest_notice_sender_id": "12345",
		"latest_notice_read_num":  "80",
		"lucky_word":              "幸运字符",
	} {

		if got := params[key]; got != want {
			t.Fatalf("params[%q] = %q, want %q", key, got, want)
		}
	}
	if got := params["latest_notice_text"]; !strings.HasSuffix(got, "…") {
		t.Fatalf("latest_notice_text should be truncated with ellipsis: %q", got)
	}
	for _, removedKey := range []string{"bind_guild_id", "group_aio_bind_guild_id", "latest_notice_image_count", "can_at_all", "remain_at_all_count_for_group", "remain_at_all_count_for_uin", "has_group_custom_portrait", "essential_msg_switch", "invite_robot_switch", "group_bind_guild_switch", "flame_switch_state", "flame_day_nums"} {
		if _, exists := params[removedKey]; exists {
			t.Fatalf("params should not contain %q", removedKey)
		}
	}
}

func TestFetchGroupPayloadCollectsOptionalData(t *testing.T) {
	host := &stubHost{responses: map[string]ob11.APIResponse{
		"get_group_detail_info": {
			Status:  "ok",
			RetCode: 0,
			Data: json.RawMessage(`{
				"group_id":123,
				"group_name":"测试群",
				"memberNum":88,
				"maxMemberNum":200,
				"groupMemo":"群介绍",
				"groupCreateTime":1700000000,
				"groupGrade":5,
				"activeMemberNum":66,
				"fingerMemo":"群规",
				"groupQuestion":"问题",
				"ownerUin":222,
				"shutUpAllTimestamp":1
			}`),
		},
		"get_group_info": {
			Status:  "ok",
			RetCode: 0,
			Data:    json.RawMessage(`{"group_id":123,"group_name":"测试群","member_count":88,"max_member_count":200,"group_all_shut":1,"group_remark":"群备注"}`),
		},
		"get_group_member_list": {
			Status:  "ok",
			RetCode: 0,
			Data: json.RawMessage(`[
				{"user_id":222,"role":"owner","sex":"male","card":"群主卡","title":"头衔","last_sent_time":99999999999},
				{"user_id":333,"role":"admin","sex":"female","is_robot":true,"unfriendly":true,"shut_up_timestamp":99999999999}
			]`),
		},
		"get_group_honor_info": {
			Status:  "ok",
			RetCode: 0,
			Data:    json.RawMessage(`{"group_id":123,"current_talkative":{"user_id":222,"nickname":"龙王"},"talkative_list":[{"user_id":333,"nickname":"群聊之火"}],"performer_list":[{"user_id":444,"nickname":"炽焰"}],"legend_list":[],"emotion_list":[],"strong_newbie_list":[]}`),
		},
		"_get_group_notice": {
			Status:  "ok",
			RetCode: 0,
			Data:    json.RawMessage(`[{"sender_id":222,"publish_time":1700000200,"notice_id":"n1","message":{"text":"公告内容","image":["a"],"images":["a","b"]},"read_num":18}]`),
		},
		"get_group_at_all_remain": {
			Status:  "ok",
			RetCode: 0,
			Data:    json.RawMessage(`{"can_at_all":true,"remain_at_all_count_for_group":5,"remain_at_all_count_for_uin":2}`),
		},
		"get_group_info_ex": {
			Status:  "ok",
			RetCode: 0,
			Data:    json.RawMessage(`{"groupCode":"123","resultCode":0,"extInfo":{"luckyWord":"Lucky","hasGroupCustomPortrait":1,"bindGuildId":"guild-1","groupOwnerId":{"memberUin":"222"},"essentialMsgSwitch":0,"inviteRobotSwitch":1,"qqMusicMedalSwitch":0,"showPlayTogetherSwitch":1,"groupBindGuildSwitch":1,"groupAioBindGuildId":"guild-aio","fullGroupExpansionSwitch":0,"inviteRobotMemberSwitch":1,"inviteRobotMemberExamine":0,"groupSquareSwitch":1,"groupExtFlameData":{"switchState":1,"state":2,"dayNums":["1","2"],"isDisplayDayNum":true}}}`),
		},
	}}

	payload, err := fetchGroupPayload(context.Background(), host, 123)
	if err != nil {
		t.Fatalf("fetchGroupPayload() error = %v", err)
	}
	if payload.Name != "测试群" || payload.Remark != "群备注" {
		t.Fatalf("unexpected base payload: %#v", payload)
	}
	if payload.AdminCount != 1 || payload.RobotCount != 1 || payload.UnfriendlyCount != 1 {
		t.Fatalf("unexpected member stats: %#v", payload)
	}
	if payload.CurrentTalkative == "" || payload.LatestNoticeText != "公告内容" {
		t.Fatalf("unexpected optional data: %#v", payload)
	}
	if payload.CanAtAll != nil || payload.HasGroupCustomPortrait != nil {
		t.Fatalf("removed optional fields should stay empty: %#v", payload)
	}
}
