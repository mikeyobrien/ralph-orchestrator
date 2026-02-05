# Data Analysis Script with Hats

This example demonstrates using Hats to create a data analysis script with pandas, visualization, and reporting.

## Task Description

Create a Python data analysis script that:
- Loads and cleans CSV data
- Performs statistical analysis
- Creates visualizations
- Generates HTML report

## PROMPT.md File

```markdown
# Task: Build Sales Data Analysis Script

Create a Python script to analyze sales data with the following requirements:

## Data Processing

1. Load sales data from CSV file
2. Clean and validate data:
   - Handle missing values
   - Convert data types
   - Remove duplicates
   - Validate date ranges

## Analysis Requirements

1. **Sales Metrics**
   - Total revenue by month
   - Average order value
   - Top 10 products by revenue
   - Sales growth rate

2. **Customer Analysis**
   - Customer segmentation (RFM analysis)
   - Customer lifetime value
   - Repeat purchase rate
   - Geographic distribution

3. **Product Analysis**
   - Best/worst performing products
   - Product category performance
   - Seasonal trends
   - Inventory turnover

## Visualizations

Create the following charts:
1. Monthly revenue trend (line chart)
2. Product category breakdown (pie chart)
3. Customer distribution map (geographic)
4. Sales heatmap by day/hour
5. Top products bar chart

## Output

Generate an HTML report with:
- Executive summary
- Key metrics dashboard
- Interactive charts (using plotly)
- Data tables
- Insights and recommendations

## File Structure

```
sales-analysis/
├── analyze.py          # Main analysis script
├── data_loader.py      # Data loading and cleaning
├── analysis.py         # Analysis functions
├── visualizations.py   # Chart generation
├── report_generator.py # HTML report creation
├── requirements.txt    # Dependencies
├── config.yaml        # Configuration
├── templates/         # HTML templates
│   └── report.html
├── data/             # Data directory
│   └── sales.csv     # Sample data
└── output/           # Output directory
    └── report.html   # Generated report
```

## Sample Data Structure

CSV columns:
- order_id, customer_id, product_id, product_name, category
- quantity, unit_price, total_price, discount
- order_date, ship_date, region, payment_method

<!-- The orchestrator will continue iterations until limits are reached -->
```

## Running Hats

```bash
hats init
cp data-analysis-prompt.md PROMPT.md
hats run --agent claude --max-iterations 35
```

## Expected Output

### analyze.py (Main Script)

```python
#!/usr/bin/env python3
"""
Sales Data Analysis Script
Analyzes sales data and generates comprehensive HTML report
"""

import pandas as pd
import numpy as np
from datetime import datetime
import yaml
import logging
from pathlib import Path

from data_loader import DataLoader
from analysis import SalesAnalyzer, CustomerAnalyzer, ProductAnalyzer
from visualizations import ChartGenerator
from report_generator import ReportGenerator

# Setup logging
logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s - %(levelname)s - %(message)s'
)
logger = logging.getLogger(__name__)

def load_config(config_path='config.yaml'):
    """Load configuration from YAML file"""
    with open(config_path, 'r') as f:
        return yaml.safe_load(f)

def main():
    """Main analysis pipeline"""
    logger.info("Starting sales data analysis...")
    
    # Load configuration
    config = load_config()
    
    # Step 1: Load and clean data
    logger.info("Loading data...")
    loader = DataLoader(config['data']['input_file'])
    df = loader.load_and_clean()
    logger.info(f"Loaded {len(df)} records")
    
    # Step 2: Perform analysis
    logger.info("Performing analysis...")
    
    # Sales analysis
    sales_analyzer = SalesAnalyzer(df)
    sales_metrics = {
        'total_revenue': sales_analyzer.calculate_total_revenue(),
        'monthly_revenue': sales_analyzer.get_monthly_revenue(),
        'avg_order_value': sales_analyzer.calculate_avg_order_value(),
        'growth_rate': sales_analyzer.calculate_growth_rate(),
        'top_products': sales_analyzer.get_top_products(10)
    }
    
    # Customer analysis
    customer_analyzer = CustomerAnalyzer(df)
    customer_metrics = {
        'total_customers': customer_analyzer.count_unique_customers(),
        'repeat_rate': customer_analyzer.calculate_repeat_rate(),
        'rfm_segments': customer_analyzer.perform_rfm_analysis(),
        'lifetime_value': customer_analyzer.calculate_clv(),
        'geographic_dist': customer_analyzer.get_geographic_distribution()
    }
    
    # Product analysis
    product_analyzer = ProductAnalyzer(df)
    product_metrics = {
        'category_performance': product_analyzer.analyze_categories(),
        'seasonal_trends': product_analyzer.find_seasonal_trends(),
        'inventory_turnover': product_analyzer.calculate_turnover(),
        'product_ranking': product_analyzer.rank_products()
    }
    
    # Step 3: Generate visualizations
    logger.info("Creating visualizations...")
    chart_gen = ChartGenerator(df)
    
    charts = {
        'revenue_trend': chart_gen.create_revenue_trend(
            sales_metrics['monthly_revenue']
        ),
        'category_pie': chart_gen.create_category_pie(
            product_metrics['category_performance']
        ),
        'customer_map': chart_gen.create_customer_map(
            customer_metrics['geographic_dist']
        ),
        'sales_heatmap': chart_gen.create_sales_heatmap(),
        'top_products_bar': chart_gen.create_top_products_bar(
            sales_metrics['top_products']
        )
    }
    
    # Step 4: Generate report
    logger.info("Generating HTML report...")
    report_gen = ReportGenerator()
    
    report_data = {
        'generated_at': datetime.now().strftime('%Y-%m-%d %H:%M:%S'),
        'record_count': len(df),
        'date_range': f"{df['order_date'].min()} to {df['order_date'].max()}",
        'sales_metrics': sales_metrics,
        'customer_metrics': customer_metrics,
        'product_metrics': product_metrics,
        'charts': charts,
        'insights': generate_insights(sales_metrics, customer_metrics, product_metrics)
    }
    
    output_path = Path(config['output']['report_path'])
    output_path.parent.mkdir(parents=True, exist_ok=True)
    
    report_gen.generate(report_data, output_path)
    
    logger.info(f"Report generated: {output_path}")
    logger.info("Analysis complete!")
    
    return output_path

def generate_insights(sales, customers, products):
    """Generate insights from analysis results"""
    insights = []
    
    # Revenue insights
    if sales['growth_rate'] > 0:
        insights.append(f"Revenue is growing at {sales['growth_rate']:.1f}% monthly")
    else:
        insights.append(f"Revenue declined by {abs(sales['growth_rate']):.1f}% monthly")
    
    # Customer insights
    repeat_rate = customers['repeat_rate']
    if repeat_rate > 0.3:
        insights.append(f"Strong customer loyalty with {repeat_rate:.1%} repeat rate")
    else:
        insights.append(f"Opportunity to improve retention (current: {repeat_rate:.1%})")
    
    # Product insights
    top_category = products['category_performance'].iloc[0]
    insights.append(f"{top_category['category']} is the top performing category")
    
    return insights

if __name__ == '__main__':
    try:
        report_path = main()
        print(f"\n✅ Analysis complete! Report saved to: {report_path}")
    except Exception as e:
        logger.error(f"Analysis failed: {e}")
        raise
```

### data_loader.py

```python
import pandas as pd
import numpy as np
from datetime import datetime
import logging

logger = logging.getLogger(__name__)

class DataLoader:
    """Handle data loading and cleaning"""
    
    def __init__(self, filepath):
        self.filepath = filepath
    
    def load_and_clean(self):
        """Load CSV and perform cleaning"""
        # Load data
        df = pd.read_csv(self.filepath)
        logger.info(f"Loaded {len(df)} raw records")
        
        # Clean data
        df = self.remove_duplicates(df)
        df = self.handle_missing_values(df)
        df = self.convert_data_types(df)
        df = self.validate_data(df)
        
        logger.info(f"Cleaned data: {len(df)} records")
        return df
    
    def remove_duplicates(self, df):
        """Remove duplicate records"""
        before = len(df)
        df = df.drop_duplicates(subset=['order_id'])
        after = len(df)
        
        if before > after:
            logger.info(f"Removed {before - after} duplicate records")
        
        return df
    
    def handle_missing_values(self, df):
        """Handle missing values appropriately"""
        # Fill numeric columns with 0
        numeric_cols = df.select_dtypes(include=[np.number]).columns
        df[numeric_cols] = df[numeric_cols].fillna(0)
        
        # Fill categorical columns with 'Unknown'
        categorical_cols = df.select_dtypes(include=['object']).columns
        df[categorical_cols] = df[categorical_cols].fillna('Unknown')
        
        return df
    
    def convert_data_types(self, df):
        """Convert columns to appropriate data types"""
        # Convert dates
        date_columns = ['order_date', 'ship_date']
        for col in date_columns:
            if col in df.columns:
                df[col] = pd.to_datetime(df[col], errors='coerce')
        
        # Convert numeric columns
        numeric_columns = ['quantity', 'unit_price', 'total_price', 'discount']
        for col in numeric_columns:
            if col in df.columns:
                df[col] = pd.to_numeric(df[col], errors='coerce')
        
        # Convert IDs to string
        id_columns = ['order_id', 'customer_id', 'product_id']
        for col in id_columns:
            if col in df.columns:
                df[col] = df[col].astype(str)
        
        return df
    
    def validate_data(self, df):
        """Validate data integrity"""
        # Remove rows with invalid dates
        if 'order_date' in df.columns:
            df = df[df['order_date'].notna()]
        
        # Remove rows with negative prices
        if 'total_price' in df.columns:
            df = df[df['total_price'] >= 0]
        
        # Remove rows with invalid quantities
        if 'quantity' in df.columns:
            df = df[df['quantity'] > 0]
        
        return df
    
    def generate_sample_data(self, num_records=1000):
        """Generate sample sales data for testing"""
        np.random.seed(42)
        
        # Generate dates
        dates = pd.date_range(
            start='2023-01-01',
            end='2023-12-31',
            periods=num_records
        )
        
        # Categories and products
        categories = ['Electronics', 'Clothing', 'Books', 'Home', 'Sports']
        products = {
            'Electronics': ['Laptop', 'Phone', 'Tablet', 'Headphones'],
            'Clothing': ['Shirt', 'Pants', 'Jacket', 'Shoes'],
            'Books': ['Fiction', 'Non-fiction', 'TextBook', 'Magazine'],
            'Home': ['Furniture', 'Decor', 'Kitchen', 'Bedding'],
            'Sports': ['Equipment', 'Apparel', 'Footwear', 'Accessories']
        }
        
        # Generate records
        records = []
        for i in range(num_records):
            category = np.random.choice(categories)
            product = np.random.choice(products[category])
            quantity = np.random.randint(1, 10)
            unit_price = np.random.uniform(10, 500)
            discount = np.random.uniform(0, 0.3)
            
            records.append({
                'order_id': f'ORD{i:05d}',
                'customer_id': f'CUST{np.random.randint(1, 200):04d}',
                'product_id': f'PROD{np.random.randint(1, 50):03d}',
                'product_name': product,
                'category': category,
                'quantity': quantity,
                'unit_price': unit_price,
                'total_price': quantity * unit_price * (1 - discount),
                'discount': discount,
                'order_date': dates[i],
                'ship_date': dates[i] + pd.Timedelta(days=np.random.randint(1, 7)),
                'region': np.random.choice(['North', 'South', 'East', 'West']),
                'payment_method': np.random.choice(['Credit Card', 'PayPal', 'Cash'])
            })
        
        return pd.DataFrame(records)
```

### visualizations.py

```python
import plotly.graph_objects as go
import plotly.express as px
from plotly.subplots import make_subplots
import pandas as pd

class ChartGenerator:
    """Generate interactive charts using Plotly"""
    
    def __init__(self, df):
        self.df = df
    
    def create_revenue_trend(self, monthly_revenue):
        """Create monthly revenue trend line chart"""
        fig = go.Figure()
        
        fig.add_trace(go.Scatter(
            x=monthly_revenue.index,
            y=monthly_revenue.values,
            mode='lines+markers',
            name='Revenue',
            line=dict(color='#1f77b4', width=3),
            marker=dict(size=8)
        ))
        
        fig.update_layout(
            title='Monthly Revenue Trend',
            xaxis_title='Month',
            yaxis_title='Revenue ($)',
            hovermode='x unified',
            template='plotly_white'
        )
        
        return fig.to_html(include_plotlyjs='cdn')
    
    def create_category_pie(self, category_data):
        """Create category breakdown pie chart"""
        fig = px.pie(
            category_data,
            values='revenue',
            names='category',
            title='Revenue by Category',
            color_discrete_sequence=px.colors.qualitative.Set3
        )
        
        fig.update_traces(
            textposition='inside',
            textinfo='percent+label'
        )
        
        return fig.to_html(include_plotlyjs='cdn')
    
    def create_sales_heatmap(self):
        """Create sales heatmap by day and hour"""
        # Extract day and hour
        self.df['day_of_week'] = self.df['order_date'].dt.day_name()
        self.df['hour'] = self.df['order_date'].dt.hour
        
        # Aggregate sales
        heatmap_data = self.df.groupby(['day_of_week', 'hour'])[
            'total_price'
        ].sum().reset_index()
        
        # Pivot for heatmap
        pivot_table = heatmap_data.pivot(
            index='day_of_week',
            columns='hour',
            values='total_price'
        )
        
        # Reorder days
        days_order = ['Monday', 'Tuesday', 'Wednesday', 'Thursday', 
                     'Friday', 'Saturday', 'Sunday']
        pivot_table = pivot_table.reindex(days_order)
        
        fig = go.Figure(data=go.Heatmap(
            z=pivot_table.values,
            x=pivot_table.columns,
            y=pivot_table.index,
            colorscale='Viridis',
            text=pivot_table.values.round(0),
            texttemplate='%{text}',
            textfont={"size": 10}
        ))
        
        fig.update_layout(
            title='Sales Heatmap by Day and Hour',
            xaxis_title='Hour of Day',
            yaxis_title='Day of Week',
            template='plotly_white'
        )
        
        return fig.to_html(include_plotlyjs='cdn')
    
    def create_top_products_bar(self, top_products):
        """Create horizontal bar chart of top products"""
        fig = go.Figure(go.Bar(
            x=top_products['revenue'],
            y=top_products['product_name'],
            orientation='h',
            marker_color='lightblue',
            text=top_products['revenue'].round(0),
            textposition='outside'
        ))
        
        fig.update_layout(
            title='Top 10 Products by Revenue',
            xaxis_title='Revenue ($)',
            yaxis_title='Product',
            template='plotly_white',
            height=400
        )
        
        return fig.to_html(include_plotlyjs='cdn')
    
    def create_customer_map(self, geographic_data):
        """Create geographic distribution map"""
        # For simplicity, create a bar chart by region
        fig = px.bar(
            geographic_data,
            x='region',
            y='customer_count',
            title='Customer Distribution by Region',
            color='customer_count',
            color_continuous_scale='Blues'
        )
        
        fig.update_layout(
            xaxis_title='Region',
            yaxis_title='Number of Customers',
            template='plotly_white',
            showlegend=False
        )
        
        return fig.to_html(include_plotlyjs='cdn')
```

## Report Template

### templates/report.html

```html
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Sales Analysis Report</title>
    <style>
        body {
            font-family: 'Segoe UI', Tahoma, Geneva, Verdana, sans-serif;
            margin: 0;
            padding: 20px;
            background: #f5f5f5;
        }
        .container {
            max-width: 1200px;
            margin: 0 auto;
            background: white;
            padding: 30px;
            border-radius: 10px;
            box-shadow: 0 2px 10px rgba(0,0,0,0.1);
        }
        h1 {
            color: #333;
            border-bottom: 3px solid #4CAF50;
            padding-bottom: 10px;
        }
        .metrics-grid {
            display: grid;
            grid-template-columns: repeat(auto-fit, minmax(250px, 1fr));
            gap: 20px;
            margin: 30px 0;
        }
        .metric-card {
            background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
            color: white;
            padding: 20px;
            border-radius: 10px;
            text-align: center;
        }
        .metric-value {
            font-size: 2em;
            font-weight: bold;
            margin: 10px 0;
        }
        .metric-label {
            font-size: 0.9em;
            opacity: 0.9;
        }
        .chart-container {
            margin: 30px 0;
        }
        .insights {
            background: #e8f5e9;
            padding: 20px;
            border-radius: 10px;
            margin: 30px 0;
        }
        .insight-item {
            margin: 10px 0;
            padding-left: 20px;
            position: relative;
        }
        .insight-item:before {
            content: "→";
            position: absolute;
            left: 0;
            color: #4CAF50;
            font-weight: bold;
        }
    </style>
</head>
<body>
    <div class="container">
        <h1>📊 Sales Analysis Report</h1>
        
        <div class="report-meta">
            <p><strong>Generated:</strong> {{ generated_at }}</p>
            <p><strong>Data Range:</strong> {{ date_range }}</p>
            <p><strong>Total Records:</strong> {{ record_count }}</p>
        </div>
        
        <h2>Key Metrics</h2>
        <div class="metrics-grid">
            <div class="metric-card">
                <div class="metric-label">Total Revenue</div>
                <div class="metric-value">${{ total_revenue|round(0) }}</div>
            </div>
            <div class="metric-card">
                <div class="metric-label">Avg Order Value</div>
                <div class="metric-value">${{ avg_order_value|round(2) }}</div>
            </div>
            <div class="metric-card">
                <div class="metric-label">Total Customers</div>
                <div class="metric-value">{{ total_customers }}</div>
            </div>
            <div class="metric-card">
                <div class="metric-label">Repeat Rate</div>
                <div class="metric-value">{{ repeat_rate|round(1) }}%</div>
            </div>
        </div>
        
        <h2>Insights</h2>
        <div class="insights">
            {% for insight in insights %}
            <div class="insight-item">{{ insight }}</div>
            {% endfor %}
        </div>
        
        <h2>Revenue Trend</h2>
        <div class="chart-container">
            {{ revenue_trend_chart|safe }}
        </div>
        
        <h2>Category Performance</h2>
        <div class="chart-container">
            {{ category_pie_chart|safe }}
        </div>
        
        <h2>Top Products</h2>
        <div class="chart-container">
            {{ top_products_chart|safe }}
        </div>
        
        <h2>Sales Patterns</h2>
        <div class="chart-container">
            {{ sales_heatmap|safe }}
        </div>
    </div>
</body>
</html>
```

## Tips for Data Analysis Tasks

1. **Specify Data Structure**: Clearly define input data format
2. **List Required Analyses**: Be specific about calculations needed
3. **Request Visualizations**: Specify chart types and libraries
4. **Output Format**: Define report structure and format
5. **Error Handling**: Request validation and error handling

## Cost Estimation

- **Iterations**: ~25-35 for complete implementation
- **Time**: ~12-18 minutes
- **Agent**: Claude recommended for complex analysis
- **API Calls**: ~$0.25-0.35