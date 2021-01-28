(ns vogelsang.core
  (:require
   [taoensso.encore :as e]
   [clojure.tools.cli :as cli]
   [taoensso.timbre :as timbre]
   [vogelsang.data :as data]
   [vogelsang.allocation]
   [vogelsang.quant]
   [cuerdas.core :as str]
   [uncomplicate.fluokitten.jvm]
   [meander.epsilon :as m]
   [vogelsang.api :as api])
  (:gen-class))

(def version [0 0 1])
(def home-dir ^String (System/getProperty "user.home"))

(def cli-opts
  [["-v" "--version"                             "print version number"]
   [nil  "--verbose"                             "print debug info"]
   [nil  "--reset-db"                            "remove all data from db"]
   [nil  "--set-symbols-to-download SYMBOLS"     "set symbols to download"]
   [nil  "--add-symbols-to-download SYMBOLS"     "add symbols to download"]
   [nil  "--remove-symbols-to-download SYMBOLS"  "add symbols to download"]
   [nil  "--list-symbols-to-download"            "list symbols to download"]
   ["-r" "--refresh-symbol SYMBOL"               "update symbol data"]
   ["-a" "--refresh-symbols"                     "update all symbols data"]
   ["-d" "--delete-symbol SYMBOL"                "delete symbol data"]
   [nil  "--check-downloaded-symbols"            "check if all data downloaded correctly"]
   [nil  "--remove-unused-symbols-data"          "delete all unnecessery data"]
   ["-h" "--help"                                "show this help"]])

(def portfolio-opts
  [["-e" "--exclude IN" "symbols to exclude"
    :default []
    :parse-fn #(str/split % ",")]
   ["-s" "--min-sharpe VAL" "symbol minimum sharpe ratio"
    :default 1.0
    :parse-fn e/as-?int]
   ["-d" "--max-dd VAL" "symbol maximum drowdown price"
    :default 0.3
    :parse-fn e/as-?int]
   ["-a" "--min-allocation VAL" "symbol minimum single redp allocation"
    :default 1.0
    :parse-fn e/as-?int]
   ["-r" "--min-redp VAL" "symbol minimum redp value"
    :default 1.0
    :parse-fn e/as-?int]
   ["-p" "--max-price VAL" "symbol maximum price"
    :parse-fn e/as-?float]
   ["-n" "--max-count VAL" "maximum symbols in portfolio"
    :default 10
    :parse-fn e/as-?float]
   ["-m" "--money VAL" "money to allocate"
    :default  100000.0
    :parse-fn e/as-?float]
   [nil "--drop-last-month" ""
    :default  false]
   ["-h" "--help" "show this help"]])

(defn usage [opts]
  (->> opts
       (mapv (fn [[s l d]]
               (str (or s "  ") "    " l \newline "        " d)))
       (str/join (str \newline \newline))))

(-main "portfolio" "-h")

(defn -main [& args]
  (let [opts (cli/parse-opts args cli-opts :in-order true)]
    (m/match opts
      {:options {:verbose true}} (timbre/set-level! :debug)
      _ (timbre/set-level! :info))

    (m/match opts
      {:options {:help true}}    (println (str (usage cli-opts)
                                               "\n\n      portfolio"))
      {:options {:version true}} (println (str/join "." version))

      {:options {:set-symbols-to-download (m/some ?symbols)}}
      (let [symbols (->> (str/split ?symbols #",|\s") (filterv not-empty))]
        (vogelsang.data/set-symbols-to-download symbols))

      {:options {:add-symbols-to-download (m/some ?symbols)}}
      (let [symbols (->> (str/split ?symbols #",|\s") (filterv not-empty))]
        (doseq [symbol symbols]
          (vogelsang.data/add-symbol-to-download symbol)))

      {:options {:remove-symbols-to-download (m/some ?symbols)}}
      (let [symbols (->> (str/split ?symbols #",|\s") (filterv not-empty))]
        (doseq [symbol symbols]
          (vogelsang.data/remove-symbol-to-download symbol)))

      {:options {:list-symbols-to-download true}}
      (println (vogelsang.data/symbols-to-download))

      {:options {:refresh-symbol (m/some ?symbol)}}
      (do
        (api/refresh-symbol! ?symbol)
        (api/refresh-quant! ?symbol))

      {:options {:delete-symbol (m/some ?symbol)}}
      (api/delete-symbol! ?symbol)

      {:options {:refresh-symbols true}}
      (do (api/refresh-symbols!)
          (api/refresh-quants!))

      {:options {:check-downloaded-symbols true}}
      (api/check-downloaded-symbols!)

      {:arguments ["portfolio" & ?args]}
      (let [popts (cli/parse-opts ?args portfolio-opts)]
        (m/match popts
          {:options {:help true}}
          (println (usage portfolio-opts))
          _
          (api/portfolio (:options popts))))

      _ "non exhaustive pattern match")))
