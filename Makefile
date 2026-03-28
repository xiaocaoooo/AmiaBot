GO ?= go
BIN_DIR ?= plugins
PLUGINS := \
	nyanyabot-plugin-amiabot-bilibili \
	nyanyabot-plugin-amiabot-pixiv \
	nyanyabot-plugin-amiabot-pjsk-card \
	nyanyabot-plugin-amiabot-pjsk-event \
	nyanyabot-plugin-amiabot-pjsk-song \
	nyanyabot-plugin-amiabot-zeabur-status

.PHONY: build test fmt clean tidy \
	build-bilibili build-pixiv build-card build-event build-song build-zeabur

build: $(addprefix $(BIN_DIR)/,$(PLUGINS))

$(BIN_DIR):
	mkdir -p $(BIN_DIR)

$(BIN_DIR)/%: | $(BIN_DIR)
	$(GO) build -o $@ ./cmd/$*

build-bilibili: $(BIN_DIR)/nyanyabot-plugin-amiabot-bilibili
build-pixiv: $(BIN_DIR)/nyanyabot-plugin-amiabot-pixiv
build-card: $(BIN_DIR)/nyanyabot-plugin-amiabot-pjsk-card
build-event: $(BIN_DIR)/nyanyabot-plugin-amiabot-pjsk-event
build-song: $(BIN_DIR)/nyanyabot-plugin-amiabot-pjsk-song
build-zeabur: $(BIN_DIR)/nyanyabot-plugin-amiabot-zeabur-status

test:
	$(GO) test ./...

fmt:
	$(GO) fmt ./...

tidy:
	$(GO) mod tidy

clean:
	rm -rf $(BIN_DIR)
