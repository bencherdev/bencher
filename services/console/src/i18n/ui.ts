export const showDefaultLang = false;

export const langButtonId = "lang-button";
export const langBoxId = "lang-box";

export enum Language {
	de = "de",
	en = "en",
	es = "es",
	fr = "fr",
	ja = "ja",
	ko = "ko",
	pt = "pt",
	ru = "ru",
	zh = "zh",
}
export const defaultLang = Language.en;

// As of right now these are the i18n languages for the GitHub Docs
// https://github.blog/2020-07-02-how-we-launched-docs-github-com/#internationalized-docs
export const otherLanguages = [
	Language.zh,
	Language.es,
	Language.pt,
	Language.ru,
	Language.ja,
	Language.fr,
	Language.de,
	Language.ko,
];

export const allLanguages = [Language.en, ...otherLanguages];

export const alsoIn = (lang: Language) => {
	switch (lang) {
		case Language.en:
			return "Also available in:";
		case Language.de:
			return "Auch erhältlich in:";
		case Language.es:
			return "También disponible en:";
		case Language.fr:
			return "Également disponible en:";
		case Language.ja:
			return "以下でも利用可能です:";
		case Language.ko:
			return "다음에서도 사용 가능:";
		case Language.pt:
			return "Também disponível em:";
		case Language.ru:
			return "Также доступно в:";
		case Language.zh:
			return "也可用于:";
	}
};

export const languageName = (lang: Language) => {
	switch (lang) {
		case Language.en:
			return "English";
		case Language.de:
			return "Deutsch";
		case Language.es:
			return "Español";
		case Language.fr:
			return "Français";
		case Language.ja:
			return "日本語";
		case Language.ko:
			return "한국어";
		case Language.pt:
			return "Português do Brasil";
		case Language.ru:
			return "Русский";
		case Language.zh:
			return "简体中文";
	}
};

export const tutorial = (lang: Language) => {
	switch (lang) {
		case Language.en:
			return "Tutorial";
		case Language.de:
			return "Lernprogramm";
		case Language.es:
			return "Tutorial";
		case Language.fr:
			return "Didacticiel";
		case Language.ja:
			return "チュートリアル";
		case Language.ko:
			return "자습서 작";
		case Language.pt:
			return "Tutorial";
		case Language.ru:
			return "учебника";
		case Language.zh:
			return "教程";
	}
};
export const signupId = "sign-up-bencher-cloud";

export const howTo = (lang: Language) => {
	switch (lang) {
		case Language.en:
			return "How To";
		case Language.de:
			return "Wie man";
		case Language.es:
			return "Cómo";
		case Language.fr:
			return "Comment";
		case Language.ja:
			return "方法";
		case Language.ko:
			return "어떻게";
		case Language.pt:
			return "Como";
		case Language.ru:
			return "Как";
		case Language.zh:
			return "如何";
	}
};

export const explanation = (lang: Language) => {
	switch (lang) {
		case Language.en:
			return "Explanation";
		case Language.de:
			return "Erklärung";
		case Language.es:
			return "Explicación";
		case Language.fr:
			return "Explication";
		case Language.ja:
			return "説明";
		case Language.ko:
			return "설명";
		case Language.pt:
			return "Explicação";
		case Language.ru:
			return "Объяснение";
		case Language.zh:
			return "解释";
	}
};

export const reference = (lang: Language) => {
	switch (lang) {
		case Language.en:
			return "Reference";
		case Language.de:
			return "Verweis";
		case Language.es:
			return "Referencia";
		case Language.fr:
			return "Référence";
		case Language.ja:
			return "関連項目";
		case Language.ko:
			return "참조";
		case Language.pt:
			return "Referência";
		case Language.ru:
			return "Ссылка";
		case Language.zh:
			return "参考";
	}
};

export const architecture = (lang: Language) => {
	switch (lang) {
		case Language.en:
			return "Architecture";
		case Language.de:
			return "Architektur";
		case Language.es:
			return "Arquitectura";
		case Language.fr:
			return "Architecture";
		case Language.ja:
			return "アーキテクチャ";
		case Language.ko:
			return "아키텍처";
		case Language.pt:
			return "Arquitetura";
		case Language.ru:
			return "Архитектура";
		case Language.zh:
			return "架构";
	}
};

export const schema = (lang: Language) => {
	switch (lang) {
		case Language.en:
			return "Database Schema";
		case Language.de:
			return "Datenbankschema";
		case Language.es:
			return "Esquema de base de datos";
		case Language.fr:
			return "Schéma de base de données";
		case Language.ja:
			return "データベーススキーマ";
		case Language.ko:
			return "데이터베이스 스키마";
		case Language.pt:
			return "Esquema de Banco de Dados";
		case Language.ru:
			return "Схема базы данных";
		case Language.zh:
			return "数据库架构";
	}
};

export const benchmarking = (lang: Language) => {
	switch (lang) {
		case Language.en:
			return "Benchmarking";
		case Language.de:
			return "Benchmarking";
		case Language.es:
			return "Evaluación Comparativa";
		case Language.fr:
			return "Analyse Comparative";
		case Language.ja:
			return "ベンチマーク";
		case Language.ko:
			return "벤치마킹";
		case Language.pt:
			return "Avaliação Comparativa";
		case Language.ru:
			return "Бенчмаркинг";
		case Language.zh:
			return "标杆管理";
	}
};

export const trackInCi = (lang: Language) => {
	switch (lang) {
		case Language.en:
			return "Track in CI";
		case Language.de:
			return "Nachverfolgen in CI";
		case Language.es:
			return "Seguimiento en CI";
		case Language.fr:
			return "Suivi dans l'intégration continue (CI)";
		case Language.ja:
			return "CIでのトラック";
		case Language.ko:
			return "CI에서 추적하기";
		case Language.pt:
			return "Rastrear no CI";
		case Language.ru:
			return "Отслеживание в CI";
		case Language.zh:
			return "在 CI 中跟踪";
	}
};

export const caseStudy = (lang: Language) => {
	switch (lang) {
		case Language.en:
			return "Case Study";
		case Language.de:
			return "Fallstudie";
		case Language.es:
			return "Estudio de Caso";
		case Language.fr:
			return "Étude de cas";
		case Language.ja:
			return "事例研究";
		case Language.ko:
			return "사례 연구";
		case Language.pt:
			return "Estudo de Caso";
		case Language.ru:
			return "Кейс-стади";
		case Language.zh:
			return "案例研究";
	}
};

export const engineering = (lang: Language) => {
	switch (lang) {
		case Language.en:
			return "Engineering";
		case Language.de:
			return "Ingenieurwesen";
		case Language.es:
			return "Ingeniería";
		case Language.fr:
			return "Ingénierie";
		case Language.ja:
			return "エンジニアリング";
		case Language.ko:
			return "공학";
		case Language.pt:
			return "Engenharia";
		case Language.ru:
			return "Инженерия";
		case Language.zh:
			return "工程";
	}
};

export const search = (lang: Language) => {
	switch (lang) {
		case Language.en:
			return "Search documentation";
		case Language.de:
			return "Dokumentation durchsuchen";
		case Language.es:
			return "Buscar en la documentación";
		case Language.fr:
			return "Rechercher dans la documentation";
		case Language.ja:
			return "ドキュメントを検索";
		case Language.ko:
			return "문서 검색";
		case Language.pt:
			return "Pesquisar na documentação";
		case Language.ru:
			return "Поиск в документации";
		case Language.zh:
			return "搜索文档";
	}
};

export const selectLanguage = (lang: Language) => {
	switch (lang) {
		case Language.en:
			return "Select a language";
		case Language.de:
			return "Sprache auswählen";
		case Language.es:
			return "Seleccionar un idioma";
		case Language.fr:
			return "Choisir une langue";
		case Language.ja:
			return "言語を選択";
		case Language.ko:
			return "언어 선택";
		case Language.pt:
			return "Selecionar um idioma";
		case Language.ru:
			return "Выберите язык";
		case Language.zh:
			return "选择语言";
	}
};

export const searchPlaceholder = (lang: Language) => {
	switch (lang) {
		case Language.en:
			return "Search…";
		case Language.de:
			return "Suchen…";
		case Language.es:
			return "Buscar…";
		case Language.fr:
			return "Rechercher…";
		case Language.ja:
			return "検索…";
		case Language.ko:
			return "검색…";
		case Language.pt:
			return "Pesquisar…";
		case Language.ru:
			return "Поиск…";
		case Language.zh:
			return "搜索…";
	}
};

export const searchNoResults = (lang: Language, query: string) => {
	switch (lang) {
		case Language.en:
			return `No results found for "${query}"`;
		case Language.de:
			return `Keine Ergebnisse für „${query}" gefunden`;
		case Language.es:
			return `No se encontraron resultados para "${query}"`;
		case Language.fr:
			return `Aucun résultat trouvé pour « ${query} »`;
		case Language.ja:
			return `"${query}" の結果が見つかりません`;
		case Language.ko:
			return `"${query}"에 대한 결과가 없습니다`;
		case Language.pt:
			return `Nenhum resultado encontrado para "${query}"`;
		case Language.ru:
			return `Результаты по запросу «${query}» не найдены`;
		case Language.zh:
			return `未找到 "${query}" 的结果`;
	}
};

export const searchLoading = (lang: Language) => {
	switch (lang) {
		case Language.en:
			return "Searching…";
		case Language.de:
			return "Suche läuft…";
		case Language.es:
			return "Buscando…";
		case Language.fr:
			return "Recherche en cours…";
		case Language.ja:
			return "検索中…";
		case Language.ko:
			return "검색 중…";
		case Language.pt:
			return "Pesquisando…";
		case Language.ru:
			return "Поиск…";
		case Language.zh:
			return "搜索中…";
	}
};

export const searchEmpty = (lang: Language) => {
	switch (lang) {
		case Language.en:
			return "Start typing to search the docs.";
		case Language.de:
			return "Beginnen Sie mit der Eingabe, um die Dokumentation zu durchsuchen.";
		case Language.es:
			return "Empieza a escribir para buscar en la documentación.";
		case Language.fr:
			return "Commencez à taper pour rechercher dans la documentation.";
		case Language.ja:
			return "入力してドキュメントを検索してください。";
		case Language.ko:
			return "문서를 검색하려면 입력을 시작하세요.";
		case Language.pt:
			return "Comece a digitar para pesquisar na documentação.";
		case Language.ru:
			return "Начните вводить текст для поиска в документации.";
		case Language.zh:
			return "开始输入以搜索文档。";
	}
};

export const searchError = (lang: Language) => {
	switch (lang) {
		case Language.en:
			return "Search is temporarily unavailable.";
		case Language.de:
			return "Die Suche ist vorübergehend nicht verfügbar.";
		case Language.es:
			return "La búsqueda no está disponible temporalmente.";
		case Language.fr:
			return "La recherche est temporairement indisponible.";
		case Language.ja:
			return "検索は一時的に利用できません。";
		case Language.ko:
			return "검색을 일시적으로 사용할 수 없습니다.";
		case Language.pt:
			return "A pesquisa está temporariamente indisponível.";
		case Language.ru:
			return "Поиск временно недоступен.";
		case Language.zh:
			return "搜索暂时不可用。";
	}
};

export const searchDevStub = (lang: Language) => {
	switch (lang) {
		case Language.en:
			return "Search is only available in production builds. Run `IS_BENCHER_CLOUD=true npm run build && npm run pagefind && npm run preview`.";
		case Language.de:
			return "Die Suche ist nur in Produktions-Builds verfügbar. Führen Sie `IS_BENCHER_CLOUD=true npm run build && npm run pagefind && npm run preview` aus.";
		case Language.es:
			return "La búsqueda solo está disponible en compilaciones de producción. Ejecuta `IS_BENCHER_CLOUD=true npm run build && npm run pagefind && npm run preview`.";
		case Language.fr:
			return "La recherche n'est disponible que dans les builds de production. Exécutez `IS_BENCHER_CLOUD=true npm run build && npm run pagefind && npm run preview`.";
		case Language.ja:
			return "検索は本番ビルドでのみ利用できます。`IS_BENCHER_CLOUD=true npm run build && npm run pagefind && npm run preview` を実行してください。";
		case Language.ko:
			return "검索은 프로덕션 빌드에서만 사용할 수 있습니다. `IS_BENCHER_CLOUD=true npm run build && npm run pagefind && npm run preview`를 실행하세요.";
		case Language.pt:
			return "A pesquisa só está disponível em builds de produção. Execute `IS_BENCHER_CLOUD=true npm run build && npm run pagefind && npm run preview`.";
		case Language.ru:
			return "Поиск доступен только в производственных сборках. Запустите `IS_BENCHER_CLOUD=true npm run build && npm run pagefind && npm run preview`.";
		case Language.zh:
			return "搜索仅在生产构建中可用。请运行 `IS_BENCHER_CLOUD=true npm run build && npm run pagefind && npm run preview`。";
	}
};
