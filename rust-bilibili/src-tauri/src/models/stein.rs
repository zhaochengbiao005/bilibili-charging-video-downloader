use serde::{Deserialize, Serialize};

/// 互动视频（stein gate）整图摘要，供 UI 展示、路径选择与离线播放。
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct SteinGraph {
    pub graph_version: u64,
    pub entry_edge_id: u64,
    pub entry_cid: u64,
    pub segment_count: u32,
    /// 去重后的分片列表（整图下载用）
    pub segments: Vec<SteinSegment>,
    /// 剧情节点
    pub nodes: Vec<SteinNode>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SteinSegment {
    pub cid: u64,
    pub title: String,
    /// 首次出现该 cid 的 edge_id
    pub edge_id: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SteinNode {
    pub edge_id: u64,
    pub title: String,
    pub is_leaf: bool,
    /// 到达本节点时播放的分片 cid（入口用 entry_cid）
    #[serde(default)]
    pub play_cid: u64,
    /// 选项出现时机：距片尾的毫秒数（B站 start_time_r）
    #[serde(default)]
    pub start_time_r_ms: u64,
    /// 出现选项时是否暂停
    #[serde(default = "default_true")]
    pub pause_video: bool,
    pub choices: Vec<SteinChoice>,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SteinChoice {
    /// 下一 edge_id（= API choice.id）
    pub edge_id: u64,
    pub option: String,
    /// 跳转分片 cid（到达下一节点时播放）
    pub cid: u64,
}

/// 离线播放器用的清单（含相对路径）
#[derive(Debug, Clone, Serialize)]
pub struct SteinPlayerManifest {
    pub title: String,
    pub bvid: String,
    pub entry_edge_id: u64,
    pub nodes: Vec<SteinPlayerNode>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SteinPlayerNode {
    pub edge_id: u64,
    pub title: String,
    pub is_leaf: bool,
    pub play_cid: u64,
    /// 相对 work_dir 的视频路径，如 segments/123.mp4
    pub file: String,
    pub start_time_r_ms: u64,
    pub pause_video: bool,
    pub choices: Vec<SteinPlayerChoice>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SteinPlayerChoice {
    pub edge_id: u64,
    pub option: String,
    pub cid: u64,
}

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SteinDownloadMode {
    /// 非互动 / 不走 stein 逻辑
    #[default]
    None,
    /// 整图：下载全部去重 cid 到 segments/ + 离线播放器
    All,
    /// 单路径：按 stein_path_cids 顺序下载并拼接
    Path,
}

impl SteinGraph {
    pub fn node(&self, edge_id: u64) -> Option<&SteinNode> {
        self.nodes.iter().find(|n| n.edge_id == edge_id)
    }

    /// 根据从根开始的 choice edge 序列，解析出有序 cid 列表（含入口）。
    pub fn resolve_path_cids(&self, choice_edge_ids: &[u64]) -> Result<Vec<u64>, String> {
        let mut cids = vec![self.entry_cid];
        let mut current = self.entry_edge_id;
        for &next_edge in choice_edge_ids {
            let node = self
                .node(current)
                .ok_or_else(|| format!("剧情节点 {current} 不存在"))?;
            let choice = node
                .choices
                .iter()
                .find(|c| c.edge_id == next_edge)
                .ok_or_else(|| {
                    format!("节点「{}」上不存在选项 edge_id={next_edge}", node.title)
                })?;
            if choice.cid != 0 {
                cids.push(choice.cid);
            }
            current = next_edge;
        }
        let mut deduped = Vec::new();
        for cid in cids {
            if deduped.last().copied() != Some(cid) {
                deduped.push(cid);
            }
        }
        Ok(deduped)
    }

    /// 用 cid → 相对路径 构建离线播放清单。
    pub fn to_player_manifest(
        &self,
        title: &str,
        bvid: &str,
        cid_to_file: &std::collections::HashMap<u64, String>,
    ) -> SteinPlayerManifest {
        let nodes = self
            .nodes
            .iter()
            .filter_map(|node| {
                let play_cid = if node.play_cid != 0 {
                    node.play_cid
                } else if node.edge_id == self.entry_edge_id {
                    self.entry_cid
                } else {
                    0
                };
                if play_cid == 0 {
                    return None;
                }
                let file = cid_to_file.get(&play_cid)?.clone();
                Some(SteinPlayerNode {
                    edge_id: node.edge_id,
                    title: node.title.clone(),
                    is_leaf: node.is_leaf,
                    play_cid,
                    file,
                    start_time_r_ms: node.start_time_r_ms,
                    pause_video: node.pause_video,
                    choices: node
                        .choices
                        .iter()
                        .map(|c| SteinPlayerChoice {
                            edge_id: c.edge_id,
                            option: c.option.clone(),
                            cid: c.cid,
                        })
                        .collect(),
                })
            })
            .collect();
        SteinPlayerManifest {
            title: title.to_string(),
            bvid: bvid.to_string(),
            entry_edge_id: self.entry_edge_id,
            nodes,
        }
    }
}

/// 生成自包含离线互动播放器 HTML（内嵌 manifest JSON）。
pub fn render_offline_player_html(manifest: &SteinPlayerManifest) -> Result<String, String> {
    let json = serde_json::to_string(manifest).map_err(|e| e.to_string())?;
    // 防止 </script> 打断
    let json = json.replace("</", "<\\/");
    Ok(format!(
        r#"<!DOCTYPE html>
<html lang="zh-CN">
<head>
<meta charset="UTF-8" />
<meta name="viewport" content="width=device-width, initial-scale=1" />
<title>{title} · 离线互动播放</title>
<style>
  :root {{
    --pink: #fb7299;
    --pink-h: #ff85ad;
    --bg: #0f1115;
    --panel: rgba(255,255,255,0.06);
    --text: #f3f4f6;
    --muted: #9ca3af;
  }}
  * {{ box-sizing: border-box; }}
  body {{
    margin: 0; min-height: 100vh; background: var(--bg); color: var(--text);
    font-family: "Segoe UI", "PingFang SC", "Microsoft YaHei", sans-serif;
    display: flex; flex-direction: column; align-items: center;
  }}
  header {{
    width: min(960px, 100%); padding: 16px 20px 8px;
  }}
  header h1 {{
    margin: 0 0 4px; font-size: 1.15rem; font-weight: 800;
  }}
  header p {{ margin: 0; color: var(--muted); font-size: 0.8rem; }}
  .stage {{
    width: min(960px, 100%); padding: 0 12px 24px;
  }}
  .player-wrap {{
    position: relative; background: #000; border-radius: 16px; overflow: hidden;
    box-shadow: 0 20px 50px rgba(0,0,0,0.45);
    aspect-ratio: 16/9;
  }}
  video {{ width: 100%; height: 100%; display: block; background: #000; object-fit: contain; }}
  .choices {{
    position: absolute; left: 0; right: 0; bottom: 0; padding: 16px 18px 20px;
    background: linear-gradient(transparent, rgba(0,0,0,0.82));
    display: none; flex-direction: column; gap: 10px; z-index: 5;
  }}
  .choices.show {{ display: flex; }}
  .choices .hint {{
    font-size: 0.78rem; color: #e5e7eb; margin-bottom: 2px; font-weight: 600;
  }}
  .choices button {{
    appearance: none; border: 1px solid rgba(255,255,255,0.22);
    background: rgba(251,114,153,0.18); color: #fff;
    border-radius: 999px; padding: 12px 18px; font-size: 0.95rem; font-weight: 700;
    cursor: pointer; text-align: left; transition: 0.15s ease;
    backdrop-filter: blur(8px);
  }}
  .choices button:hover {{
    background: rgba(251,114,153,0.45); border-color: var(--pink);
    transform: translateY(-1px);
  }}
  .toolbar {{
    display: flex; flex-wrap: wrap; gap: 8px; margin-top: 12px; align-items: center;
  }}
  .toolbar button, .toolbar a {{
    border: 0; border-radius: 10px; padding: 8px 14px; font-weight: 700; font-size: 0.82rem;
    cursor: pointer; text-decoration: none; color: #fff;
  }}
  .btn-pink {{ background: linear-gradient(90deg, var(--pink), var(--pink-h)); }}
  .btn-dim {{ background: var(--panel); color: var(--text); }}
  .path {{
    margin-top: 10px; font-size: 0.78rem; color: var(--muted); line-height: 1.5;
  }}
  .ending {{
    position: absolute; inset: 0; display: none; align-items: center; justify-content: center;
    flex-direction: column; gap: 14px; background: rgba(0,0,0,0.72); z-index: 6;
  }}
  .ending.show {{ display: flex; }}
  .ending h2 {{ margin: 0; font-size: 1.4rem; }}
  .err {{ color: #fca5a5; padding: 12px; font-size: 0.85rem; }}
</style>
</head>
<body>
<header>
  <h1 id="title"></h1>
  <p id="meta"></p>
</header>
<div class="stage">
  <div class="player-wrap">
    <video id="video" controls playsinline></video>
    <div class="choices" id="choices">
      <div class="hint">请选择分支（与 B 站互动视频类似）</div>
      <div id="choice-btns"></div>
    </div>
    <div class="ending" id="ending">
      <h2>本线结局</h2>
      <button class="btn-pink" id="restart-end">从头再玩</button>
    </div>
  </div>
  <div class="toolbar">
    <button class="btn-pink" id="restart">重新开始</button>
    <button class="btn-dim" id="undo" disabled>回退一步</button>
  </div>
  <div class="path" id="path"></div>
  <div class="err" id="err"></div>
</div>
<script id="manifest" type="application/json">{json}</script>
<script>
(function () {{
  const raw = document.getElementById('manifest').textContent;
  /** @type {{title:string,bvid:string,entry_edge_id:number,nodes:Array<any>}} */
  const M = JSON.parse(raw);
  const byEdge = new Map(M.nodes.map(n => [n.edge_id, n]));
  const video = document.getElementById('video');
  const choicesEl = document.getElementById('choices');
  const choiceBtns = document.getElementById('choice-btns');
  const endingEl = document.getElementById('ending');
  const pathEl = document.getElementById('path');
  const errEl = document.getElementById('err');
  const undoBtn = document.getElementById('undo');

  document.getElementById('title').textContent = M.title || '离线互动播放';
  document.getElementById('meta').textContent =
    (M.bvid || '') + ' · 本地离线 · 共 ' + M.nodes.length + ' 个可播节点';

  let history = [];
  let choiceShown = false;
  let currentEdge = M.entry_edge_id;

  function showErr(msg) {{
    errEl.textContent = msg || '';
  }}

  function updatePath() {{
    const names = history.map(h => h.title || ('edge ' + h.edge_id));
    pathEl.textContent = names.length
      ? '路径：' + names.join(' → ')
      : '路径：入口';
    undoBtn.disabled = history.length <= 1;
  }}

  function hideOverlay() {{
    choicesEl.classList.remove('show');
    endingEl.classList.remove('show');
    choiceShown = false;
  }}

  function showChoices(node) {{
    if (!node.choices || !node.choices.length) return;
    choiceBtns.innerHTML = '';
    node.choices.forEach(c => {{
      const b = document.createElement('button');
      b.type = 'button';
      b.textContent = c.option || ('选项 ' + c.edge_id);
      b.onclick = () => goTo(c.edge_id);
      choiceBtns.appendChild(b);
    }});
    choicesEl.classList.add('show');
    choiceShown = true;
    if (node.pause_video !== false) {{
      try {{ video.pause(); }} catch (_) {{}}
    }}
  }}

  function showEnding() {{
    endingEl.classList.add('show');
  }}

  function goTo(edgeId) {{
    const node = byEdge.get(edgeId);
    if (!node) {{
      showErr('找不到节点 edge_id=' + edgeId + '（可能分片未下载完整）');
      return;
    }}
    if (!node.file) {{
      showErr('节点缺少视频文件：' + (node.title || edgeId));
      return;
    }}
    hideOverlay();
    showErr('');
    currentEdge = edgeId;
    history.push({{ edge_id: edgeId, title: node.title }});
    updatePath();
    video.src = node.file;
    video.load();
    const p = video.play();
    if (p && p.catch) p.catch(() => {{}});
  }}

  function maybeOfferChoices() {{
    const node = byEdge.get(currentEdge);
    if (!node || choiceShown) return;
    if (!node.choices || !node.choices.length) {{
      if (node.is_leaf || video.ended) showEnding();
      return;
    }}
    const dur = video.duration;
    if (!isFinite(dur) || dur <= 0) return;
    const remainMs = Math.max(0, (dur - video.currentTime) * 1000);
    const trigger = typeof node.start_time_r_ms === 'number' ? node.start_time_r_ms : 800;
    // 距片尾不足 trigger 毫秒时弹出选项；极短视频则在 85% 处弹出
    const threshold = Math.min(Math.max(trigger, 200), dur * 1000 * 0.5);
    if (remainMs <= threshold || video.currentTime / dur >= 0.92) {{
      showChoices(node);
    }}
  }}

  video.addEventListener('timeupdate', maybeOfferChoices);
  video.addEventListener('ended', () => {{
    const node = byEdge.get(currentEdge);
    if (!node) return;
    if (node.choices && node.choices.length) {{
      if (!choiceShown) showChoices(node);
    }} else {{
      showEnding();
    }}
  }});
  video.addEventListener('error', () => {{
    showErr('视频加载失败：' + (video.currentSrc || '') + '。请确认用浏览器打开本目录下的 play.html，且 segments 文件夹完整。');
  }});

  document.getElementById('restart').onclick = () => {{
    history = [];
    goTo(M.entry_edge_id);
  }};
  document.getElementById('restart-end').onclick = () => {{
    history = [];
    goTo(M.entry_edge_id);
  }};
  undoBtn.onclick = () => {{
    if (history.length <= 1) return;
    history.pop();
    const prev = history.pop();
    if (prev) goTo(prev.edge_id);
    else goTo(M.entry_edge_id);
  }};

  if (!byEdge.has(M.entry_edge_id)) {{
    showErr('清单缺少入口节点，无法播放。');
  }} else {{
    goTo(M.entry_edge_id);
  }}
}})();
</script>
</body>
</html>
"#,
        title = html_escape(&manifest.title),
        json = json,
    ))
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_path_cids_follows_choices() {
        let graph = SteinGraph {
            graph_version: 1,
            entry_edge_id: 1,
            entry_cid: 100,
            segment_count: 3,
            segments: vec![],
            nodes: vec![
                SteinNode {
                    edge_id: 1,
                    title: "入口".to_string(),
                    is_leaf: false,
                    play_cid: 100,
                    start_time_r_ms: 300,
                    pause_video: true,
                    choices: vec![
                        SteinChoice {
                            edge_id: 2,
                            option: "A".to_string(),
                            cid: 200,
                        },
                        SteinChoice {
                            edge_id: 3,
                            option: "B".to_string(),
                            cid: 300,
                        },
                    ],
                },
                SteinNode {
                    edge_id: 2,
                    title: "A支线".to_string(),
                    is_leaf: true,
                    play_cid: 200,
                    start_time_r_ms: 0,
                    pause_video: true,
                    choices: vec![],
                },
            ],
        };
        let cids = graph.resolve_path_cids(&[2]).expect("path");
        assert_eq!(cids, vec![100, 200]);
        let mut map = std::collections::HashMap::new();
        map.insert(100, "segments/100.mp4".into());
        map.insert(200, "segments/200.mp4".into());
        let m = graph.to_player_manifest("t", "BVxx", &map);
        assert_eq!(m.entry_edge_id, 1);
        assert_eq!(m.nodes.len(), 2);
        let html = render_offline_player_html(&m).expect("html");
        assert!(html.contains("离线互动播放"));
        assert!(html.contains("segments/100.mp4"));
    }
}