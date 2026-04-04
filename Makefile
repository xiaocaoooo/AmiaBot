GO ?= go
BIN_DIR ?= plugins
PLUGINS := \
	nyanyabot-plugin-amiabot-bilibili \
	nyanyabot-plugin-amiabot-pixiv \
	nyanyabot-plugin-amiabot-pjsk-account \
	nyanyabot-plugin-amiabot-pjsk-bind \
	nyanyabot-plugin-amiabot-pjsk-card \
	nyanyabot-plugin-amiabot-pjsk-event \
	nyanyabot-plugin-amiabot-pjsk-song \
	nyanyabot-plugin-amiabot-pjsk-profile \
	nyanyabot-plugin-amiabot-pjsk-b30 \
	nyanyabot-plugin-amiabot-zeabur-status

.PHONY: build test fmt clean tidy \
	build-bilibili build-pixiv build-account build-bind build-card build-event build-song build-profile build-b30 build-zeabur

build: $(addprefix $(BIN_DIR)/,$(PLUGINS))

$(BIN_DIR):
	mkdir -p $(BIN_DIR)

$(BIN_DIR)/%: | $(BIN_DIR)
	$(GO) build -o $@ ./cmd/$*

build-bilibili: $(BIN_DIR)/nyanyabot-plugin-amiabot-bilibili
build-pixiv: $(BIN_DIR)/nyanyabot-plugin-amiabot-pixiv
build-account: $(BIN_DIR)/nyanyabot-plugin-amiabot-pjsk-account
build-bind: $(BIN_DIR)/nyanyabot-plugin-amiabot-pjsk-bind
build-card: $(BIN_DIR)/nyanyabot-plugin-amiabot-pjsk-card
build-event: $(BIN_DIR)/nyanyabot-plugin-amiabot-pjsk-event
build-song: $(BIN_DIR)/nyanyabot-plugin-amiabot-pjsk-song
build-profile: $(BIN_DIR)/nyanyabot-plugin-amiabot-pjsk-profile
build-b30: $(BIN_DIR)/nyanyabot-plugin-amiabot-pjsk-b30
build-zeabur: $(BIN_DIR)/nyanyabot-plugin-amiabot-zeabur-status

test:
	$(GO) test ./...

fmt:
	$(GO) fmt ./...

tidy:
	$(GO) mod tidy

clean:
	rm -rf $(BIN_DIR)
