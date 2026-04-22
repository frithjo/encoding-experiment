(function () {
  function defaultErrorMessage(err, fallback) {
    if (err && err.message) return err.message;
    return fallback;
  }

  function readJsonOrThrow(response) {
    if (!response.ok) {
      return response
        .json()
        .catch(function () { return {}; })
        .then(function (body) {
          throw new Error(body.error || ("HTTP " + response.status + " " + response.statusText));
        });
    }
    return response.json();
  }

  function readTextOrThrow(response) {
    if (!response.ok) {
      return response
        .text()
        .then(function (body) {
          throw new Error(body || ("HTTP " + response.status + " " + response.statusText));
        });
    }
    return response.text();
  }

  function ensureScope(scope) {
    if (!window.__larqlAsyncRuns) window.__larqlAsyncRuns = {};
    if (!window.__larqlAsyncRuns[scope]) {
      window.__larqlAsyncRuns[scope] = { timer: null, activeRunId: null };
    }
    return window.__larqlAsyncRuns[scope];
  }

  function clearScope(scope) {
    var state = ensureScope(scope);
    if (state.timer) {
      clearTimeout(state.timer);
      state.timer = null;
    }
  }

  function resolveResultUrl(opts) {
    if (typeof opts.resultUrlBuilder === "function") {
      return opts.resultUrlBuilder(opts.runId);
    }
    if (opts.resultUrl) return opts.resultUrl;
    return "/partials/result-panel?run_id=" + encodeURIComponent(opts.runId);
  }

  function pollRun(opts) {
    var state = ensureScope(opts.scope);
    clearScope(opts.scope);
    state.activeRunId = opts.runId;
    var attempt = 0;
    var maxAttempts = opts.maxAttempts || 120;

    if (opts.panel) opts.panel.hidden = false;
    if (opts.resultEl) opts.resultEl.innerHTML = "";
    if (opts.statusEl) opts.statusEl.textContent = "Queued - polling run " + opts.runId + "...";

    function scheduleNext() {
      attempt += 1;
      if (attempt > maxAttempts) {
        clearScope(opts.scope);
        if (opts.statusEl) opts.statusEl.textContent = "Timed out waiting for result.";
        return;
      }
      var delayMs = Math.min(500 * Math.pow(1.35, attempt - 1), 5000);
      state.timer = setTimeout(runPoll, delayMs);
    }

    function renderResult() {
      return fetch(resolveResultUrl(opts), {
        credentials: "same-origin"
      })
        .then(readTextOrThrow)
        .then(function (html) {
          if (state.activeRunId !== opts.runId) return;
          if (opts.resultEl) opts.resultEl.innerHTML = html;
          if (opts.statusEl) opts.statusEl.textContent = "Done.";
        });
    }

    function runPoll() {
      fetch("/api/runs/" + encodeURIComponent(opts.runId), {
        credentials: "same-origin",
        headers: { "accept": "application/json" }
      })
        .then(readJsonOrThrow)
        .then(function (body) {
          if (state.activeRunId !== opts.runId) return;
          var run = body.run;
          if (!run) throw new Error("Run payload missing.");
          if (run.status === "pending" || run.status === "running") {
            if (opts.statusEl) {
              opts.statusEl.textContent = "Running - " + run.status + " - poll " + attempt + "/" + maxAttempts;
            }
            scheduleNext();
            return;
          }
          clearScope(opts.scope);
          if (run.status === "error") {
            if (opts.statusEl) opts.statusEl.textContent = "Error.";
            if (opts.resultEl) {
              opts.resultEl.innerHTML = '<p class="banner error" role="alert"></p>';
              var errEl = opts.resultEl.querySelector('[role="alert"]');
              if (errEl) errEl.textContent = run.summary || "Unknown error";
            }
            return;
          }
          return renderResult();
        })
        .catch(function (err) {
          if (state.activeRunId !== opts.runId) return;
          clearScope(opts.scope);
          if (opts.statusEl) opts.statusEl.textContent = "Polling failed.";
          if (opts.resultEl) {
            opts.resultEl.innerHTML = '<p class="banner error" role="alert"></p>';
            var errEl = opts.resultEl.querySelector('[role="alert"]');
            if (errEl) errEl.textContent = defaultErrorMessage(err, "Could not load run status.");
          }
        });
    }

    runPoll();
  }

  function submitAndPoll(opts) {
    if (opts.panel) opts.panel.hidden = false;
    if (opts.statusEl) opts.statusEl.textContent = "Submitting...";
    if (opts.resultEl) opts.resultEl.innerHTML = "";
    clearScope(opts.scope);

    return fetch(opts.url, {
      method: "POST",
      credentials: "same-origin",
      headers: {
        "content-type": "application/json",
        "accept": "application/json"
      },
      body: JSON.stringify(opts.body)
    })
      .then(readJsonOrThrow)
      .then(function (body) {
        if (!body.run_id) throw new Error("No run_id");
        pollRun({
          scope: opts.scope,
          runId: body.run_id,
          panel: opts.panel,
          statusEl: opts.statusEl,
          resultEl: opts.resultEl,
          maxAttempts: opts.maxAttempts,
          resultUrl: opts.resultUrl,
          resultUrlBuilder: opts.resultUrlBuilder
        });
      })
      .catch(function (err) {
        if (opts.statusEl) opts.statusEl.textContent = "Failed.";
        if (opts.resultEl) {
          opts.resultEl.innerHTML = '<p class="banner error" role="alert"></p>';
          var errEl = opts.resultEl.querySelector('[role="alert"]');
          if (errEl) errEl.textContent = defaultErrorMessage(err, "Could not submit background run.");
        }
      });
  }

  window.LarqlAsyncRuns = {
    clearScope: clearScope,
    pollRun: pollRun,
    submitAndPoll: submitAndPoll
  };
})();
