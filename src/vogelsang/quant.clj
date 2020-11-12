(ns vogelsang.quant
  (:require
   [hanse.lubeck.quant :as quant]
   [hanse.lubeck.redp :as redp]
   [vogelsang.data :as data]
   [taoensso.encore :as e]
   [cuerdas.core :as str]
   [vogelsang.db :as db]
   [datalevin.core :as d]
   [taoensso.timbre :as timbre]))

(defn- add-fn->doc-fn
  ([symbol k fn & args]
   (e/when-lets [symbol (-> symbol name str/lower str/kebab)
                 vogelsang-id (e/merge-keywords [:yf symbol :quant])
                 ret     (data/return symbol :month 12)
                 value   (apply fn (conj (vec args) ret))
                 doc     {k value :vogelsang/id vogelsang-id :yf/symbol symbol}]
     doc)))

(defn add-sharpe-ratio
  ([symbol]
   (add-sharpe-ratio symbol {}))
  ([symbol {:keys [frisk freq]
            :or   {frisk 0.0
                   freq  12}}]
   (timbre/debug :yf.quant/sharpe-ratio (e/merge-keywords [:yf symbol :quant]))
   (when-let [doc (add-fn->doc-fn symbol :yf.quant/sharpe-ratio quant/annualized-sharpe-ratio frisk freq)]
     (d/transact! db/conn [doc]))))

(defn add-downside-risk
  ([symbol]
   (add-downside-risk symbol {}))
  ([symbol {:keys [mar]
            :or   {mar 0.0}}]
   (timbre/debug :yf.quant/downside-risk (e/merge-keywords [:yf symbol :quant]))
   (when-let [doc (add-fn->doc-fn symbol :yf.quant/downside-risk quant/downside-risk mar)]
     (d/transact! db/conn [doc]))))

(defn add-annualized-return
  ([symbol]
   (add-annualized-return symbol {}))
  ([symbol {:keys [freq mode]
            :or   {freq 12
                   mode :geometric}}]
   (timbre/debug :yf.quant/annualized-return (e/merge-keywords [:yf symbol :quant]))
   (when-let [doc (add-fn->doc-fn symbol :yf.quant/annualized-return quant/annualized-return freq mode)]
     (d/transact! db/conn [doc]))))

(defn add-annualized-risk
  ([symbol]
   (add-annualized-risk symbol {}))
  ([symbol {:keys [freq]
            :or   {freq 12}}]
   (timbre/debug :yf.quant/annualized-risk (e/merge-keywords [:yf symbol :quant]))
   (when-let [doc (add-fn->doc-fn symbol :yf.quant/annualized-risk quant/annualized-risk freq)]
     (d/transact! db/conn [doc]))))

(defn add-average-drawdown [symbol]
  (timbre/debug :yf.quant/average-drawdown (e/merge-keywords [:yf symbol :quant]))
  (when-let [doc (add-fn->doc-fn symbol :yf.quant/average-drawdown quant/average-drawdown)]
    (d/transact! db/conn [doc])))

(defn add-maximum-drawdown [symbol]
  (timbre/debug :yf.quant/maximum-drawdown (e/merge-keywords [:yf symbol :quant]))
  (when-let [doc (add-fn->doc-fn symbol :yf.quant/maximum-drawdown quant/maximum-drawdown)]
    (d/transact! db/conn [doc])))

(defn add-rate-of-return [symbol]
  (timbre/debug :yf.quant/rate-of-return (e/merge-keywords [:yf symbol :quant]))
  (when-let [doc (add-fn->doc-fn symbol :yf.quant/rate-of-return quant/rate-of-return)]
    (d/transact! db/conn [doc])))

(defn add-cagr [symbol]
  (timbre/debug :yf.quant/cagr (e/merge-keywords [:yf symbol :quant]))
  (when-let [doc (add-fn->doc-fn symbol :yf.quant/cagr quant/cagr 12)]
    (d/transact! db/conn [doc])))

(defn add-calmar-ratio
  ([symbol]
   (add-calmar-ratio symbol {}))
  ([symbol {:keys [frisk freq]
            :or   {frisk 0.0
                   freq  12}}]
   (timbre/debug :yf.quant/calmar-ratio (e/merge-keywords [:yf symbol :quant]))
   (when-let [doc (add-fn->doc-fn symbol :yf.quant/calmar-ratio quant/calmar-ratio frisk freq)]
     (d/transact! db/conn [doc]))))

(defn add-redp
  ([symbol]
   (add-redp symbol {}))
  ([symbol {:keys [freq]
            :or   {freq 12}}]
   (timbre/debug :yf.quant/redp (e/merge-keywords [:yf symbol :quant]))
   (when-let [doc (e/when-lets [symbol       (-> symbol name str/lower str/kebab)
                                vogelsang-id (e/merge-keywords [:yf symbol :quant])
                                close        (data/quotes symbol :month :close 12)
                                value        (quant/rolling-economic-drawndown 12 close)
                                doc          {:yf.quant/redp value :vogelsang/id vogelsang-id}]
                    doc)]
     (d/transact! db/conn [doc]))))

(defn refresh-analyse
  ([symbol]
   (refresh-analyse symbol {}))
  ([symbol opts]
   (add-sharpe-ratio symbol opts)
   (add-downside-risk symbol opts)
   (add-annualized-return symbol opts)
   (add-annualized-risk symbol opts)
   (add-annualized-risk symbol opts)
   (add-average-drawdown symbol)
   (add-maximum-drawdown symbol)
   (add-rate-of-return symbol)
   (add-cagr symbol)
   (add-calmar-ratio symbol opts)
   (add-redp symbol opts)))
