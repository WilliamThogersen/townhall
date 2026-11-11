(function (window) {
  "use strict";

  var API_VERSION = 1;
  var host = window.parent;
  var nextId = 1;
  var pending = {};
  var subscribers = [];
  var themeHandlers = [];
  var actionHandlers = [];
  var resolveReady;

  var ready = new Promise(function (resolve) {
    resolveReady = resolve;
  });

  function post(message) {
    host.postMessage(Object.assign({ th: 1 }, message), "*");
  }

  function request(type, fields) {
    var id = nextId++;
    return new Promise(function (resolve, reject) {
      pending[id] = { resolve: resolve, reject: reject };
      post(Object.assign({ id: id, type: type }, fields));
    });
  }

  function each(handlers, value) {
    handlers.forEach(function (handler) {
      handler(value);
    });
  }

  function onMessage(event) {
    var message = event.data;
    if (event.source !== host || !message || message.th !== 1) return;
    switch (message.type) {
      case "init":
        resolveReady(message);
        break;
      case "response":
        var waiting = pending[message.id];
        if (!waiting) return;
        delete pending[message.id];
        if (message.error !== undefined) waiting.reject(new Error(message.error));
        else waiting.resolve(message.result);
        break;
      case "event":
        subscribers.forEach(function (sub) {
          if (sub.topics.indexOf(message.topic) !== -1) sub.handler(message.event);
        });
        break;
      case "theme":
        each(themeHandlers, message.theme);
        break;
      case "composer.action":
        each(actionHandlers);
        break;
    }
  }

  window.addEventListener("message", onMessage);

  window.townhall = {
    apiVersion: API_VERSION,
    ready: ready,
    api: {
      call: function (method, path, body) {
        var fields = { method: method, path: path };
        if (body !== undefined) fields.body = body;
        return request("api.call", fields);
      },
    },
    events: {
      subscribe: function (topics, handler) {
        subscribers.push({ topics: topics, handler: handler });
        return request("events.subscribe", { topics: topics });
      },
    },
    ui: {
      resize: function (height) {
        return request("ui.resize", { height: height });
      },
      toast: function (text, tone) {
        return request("ui.toast", { text: text, tone: tone || "info" });
      },
      navigate: function (path) {
        return request("ui.navigate", { path: path });
      },
    },
    composer: {
      insert: function (text) {
        return request("composer.insert", { text: text });
      },
      onAction: function (handler) {
        actionHandlers.push(handler);
      },
    },
    theme: {
      onChange: function (handler) {
        themeHandlers.push(handler);
      },
    },
  };

  post({ type: "hello", apiVersion: API_VERSION });
})(window);
