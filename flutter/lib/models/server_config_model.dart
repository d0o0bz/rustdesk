import 'dart:convert';

import 'package:flutter/foundation.dart';

import 'platform_model.dart';

/// 多服务器配置条目，对应后端 `ServerConfig`。
class ServerConfigItem {
  final String id;
  final String name;
  final String idServer;
  final int idPort;
  final String relayServer;
  final int? relayPort;
  final bool isDefault;
  final bool isCurrent;
  final bool isAvailable;
  final int? avgLatency;

  ServerConfigItem({
    required this.id,
    required this.name,
    required this.idServer,
    required this.idPort,
    this.relayServer = '',
    this.relayPort,
    this.isDefault = false,
    this.isCurrent = false,
    this.isAvailable = false,
    this.avgLatency,
  });

  factory ServerConfigItem.fromJson(Map<String, dynamic> json) {
    return ServerConfigItem(
      id: json['id'] as String? ?? '',
      name: json['name'] as String? ?? '',
      idServer: json['id_server'] as String? ?? '',
      idPort: (json['id_port'] as num?)?.toInt() ?? 0,
      relayServer: json['relay_server'] as String? ?? '',
      relayPort: (json['relay_port'] as num?)?.toInt(),
      isDefault: json['is_default'] as bool? ?? false,
      isCurrent: json['is_current'] as bool? ?? false,
      isAvailable: json['is_available'] as bool? ?? false,
      avgLatency: (json['avg_latency'] as num?)?.toInt(),
    );
  }
}

/// 多服务器配置状态，封装列表加载与增删改/切换/检测/自动开关。
class ServerConfigState extends ChangeNotifier {
  List<ServerConfigItem> _configs = [];
  bool _loading = false;
  bool _autoSwitchEnabled = false;

  List<ServerConfigItem> get configs => _configs;
  bool get loading => _loading;
  bool get autoSwitchEnabled => _autoSwitchEnabled;

  /// 加载全部配置并刷新当前标记与自动开关。
  Future<void> load() async {
    _loading = true;
    notifyListeners();
    try {
      final raw = await bind.mainGetAllServerConfigs();
      final list = (jsonDecode(raw) as List)
          .map((e) => ServerConfigItem.fromJson(e as Map<String, dynamic>))
          .toList();
      _configs = list;
      _autoSwitchEnabled = await bind.mainGetAutoSwitchEnabled();
    } catch (e) {
      debugPrint('load server configs failed: $e');
      _configs = [];
    } finally {
      _loading = false;
      notifyListeners();
    }
  }

  /// 新增配置，成功返回 null，失败返回后端错误文案。
  Future<String?> add({
    required String name,
    required String idServer,
    required int idPort,
    String relayServer = '',
    int? relayPort,
  }) async {
    final ret = await bind.mainAddServerConfig(
      name: name,
      idServer: idServer,
      idPort: idPort,
      relayServer: relayServer,
      relayPort: relayPort ?? 0,
    );
    if (ret == 'ok') {
      await load();
      return null;
    }
    return ret;
  }

  /// 更新配置，成功返回 null，失败返回后端错误文案。
  Future<String?> update({
    required String id,
    required String name,
    required String idServer,
    required int idPort,
    String relayServer = '',
    int? relayPort,
  }) async {
    final ret = await bind.mainUpdateServerConfig(
      id: id,
      name: name,
      idServer: idServer,
      idPort: idPort,
      relayServer: relayServer,
      relayPort: relayPort ?? 0,
    );
    if (ret == 'ok') {
      await load();
      return null;
    }
    return ret;
  }

  /// 删除配置，成功返回 null，失败返回后端错误文案。
  Future<String?> delete(String id) async {
    final ret = await bind.mainDeleteServerConfig(id: id);
    if (ret == 'ok') {
      await load();
      return null;
    }
    return ret;
  }

  /// 切换当前配置，成功返回 null，失败返回后端错误文案。
  Future<String?> switchTo(String id) async {
    final ret = await bind.mainSwitchServerConfig(id: id);
    if (ret == 'ok') {
      await load();
      return null;
    }
    return ret;
  }

  /// 检测单个配置可用性，并把结果回填到列表，成功返回检测结果 Map，失败返回 null。
  Future<Map<String, dynamic>?> check(String id) async {
    try {
      final raw = await bind.mainCheckServerConfig(id: id);
      if (raw == 'null' || raw.isEmpty) return null;
      final result = jsonDecode(raw) as Map<String, dynamic>;
      final idx = _configs.indexWhere((c) => c.id == id);
      if (idx >= 0) {
        final old = _configs[idx];
        final available = result['id_server_status'] == 'Available';
        final latency = (result['latency'] as num?)?.toInt();
        _configs[idx] = ServerConfigItem(
          id: old.id,
          name: old.name,
          idServer: old.idServer,
          idPort: old.idPort,
          relayServer: old.relayServer,
          relayPort: old.relayPort,
          isDefault: old.isDefault,
          isCurrent: old.isCurrent,
          isAvailable: available,
          avgLatency: latency ?? old.avgLatency,
        );
        notifyListeners();
      }
      return result;
    } catch (e) {
      debugPrint('check server config failed: $e');
      return null;
    }
  }

  /// 设置自动切换开关。
  Future<void> setAutoSwitch(bool enabled) async {
    await bind.mainSetAutoSwitchEnabled(enabled: enabled);
    _autoSwitchEnabled = enabled;
    notifyListeners();
  }
}
