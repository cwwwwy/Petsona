export default function App() {
  return (
    <main className="settings">
      <header className="settings-header">
        <h1>Petsona</h1>
        <p className="hint">
          M0 占位窗口：用于验证内容窗口的焦点与输入。真正的设置页在 M3 实现。
        </p>
      </header>
      <section className="card">
        <label className="field">
          <span>测试输入</span>
          <input
            autoFocus
            placeholder="打开本窗口后应能直接输入文字（中文 IME 也应正常）"
          />
        </label>
        <p className="meta">
          宠物浮层由 Rust 原生窗口渲染，不经 WebView；本窗口是 WebView 内容页面。
        </p>
      </section>
    </main>
  );
}
